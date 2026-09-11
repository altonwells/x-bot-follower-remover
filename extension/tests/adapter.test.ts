import { test, type TestContext } from "node:test";
import assert from "node:assert/strict";
import { XClient } from "../src/x-client";
import { OPERATIONS, parseUser } from "../src/parsers";
import type { Policy } from "../src/protocol";

const policy: Policy = {
  inactive_days: 30,
  skip_verified: true,
  skip_following: true,
  include_zero_posts: true,
  delay_seconds: 10,
  batch_limit: 50,
};
const modern = (id = "2") => ({
  rest_id: id,
  core: { screen_name: `person${id}` },
  is_blue_verified: false,
  verification: { verified: false },
  privacy: { protected: false },
  relationship_counts: { followers: 0, following: 10 },
  tweet_counts: { tweets: 4 },
  relationship_perspectives: { followed_by: true, following: false },
});
const timeline = (date: string) => ({
  instructions: [
    {
      type: "TimelineAddEntries",
      entries: [
        {
          content: {
            itemContent: {
              tweet_results: {
                result: { legacy: { user_id_str: "2", created_at: date } },
              },
            },
          },
        },
      ],
    },
  ],
});
const empty = () => ({
  instructions: [
    { type: "TimelineAddEntries", entries: [] },
    { type: "TimelineTerminateTimeline", direction: "Bottom" },
  ],
});
const page = () => ({
  instructions: [
    {
      type: "TimelineAddEntries",
      entries: [
        { content: { itemContent: { user_results: { result: modern() } } } },
        { content: { cursorType: "Bottom", value: "next-page" } },
      ],
    },
  ],
});

async function fixture(
  t: TestContext,
  responder: (url: URL, init?: RequestInit) => Response | Promise<Response>,
) {
  const original = globalThis.chrome;
  let revision = "fresh",
    noSeed = false;
  const calls: { url: URL; init?: RequestInit }[] = [];
  const storage: Record<string, unknown> = {};
  Object.assign(globalThis, {
    chrome: {
      runtime: { id: "test-extension" },
      cookies: {
        get: async ({ name }: { name: string }) => ({
          value: name === "twid" ? "u=1" : "test-csrf",
        }),
      },
      storage: {
        local: { get: async () => ({}), set: async () => {} },
        session: {
          get: async () => ({ webBearer: "Bearer test-bearer" }),
          set: async (v: object) => Object.assign(storage, v),
        },
      },
      tabs: { query: async () => [{ id: 1, active: true }] },
      scripting: {
        executeScript: async () => [
          {
            result: {
              scripts: ["https://abs.twimg.com/x-web/assets/main.js"],
              inline: "",
              key: btoa(
                String.fromCharCode(...Array.from({ length: 48 }, (_, i) => i)),
              ),
              frames: noSeed
                ? []
                : Array(4).fill(
                    "M0 0 0 0 " +
                      Array(16)
                        .fill("10 20 30 40 50 60 70 80 90 100 110")
                        .join("C"),
                  ),
            },
          },
        ],
      },
    },
  });
  t.after(() => Object.assign(globalThis, { chrome: original }));
  t.mock.method(
    globalThis,
    "fetch",
    async (input: string | URL | Request, init?: RequestInit) => {
      const url = new URL(input instanceof Request ? input.url : input);
      calls.push({ url, init });
      if (url.host === "abs.twimg.com") {
        if (url.pathname.includes("sign.o"))
          return new Response("(a[2],16)*(a[12],16)*(a[14],16)*(a[7],16)");
        return new Response(
          'import("./sign.o-seed.js");' +
            OPERATIONS.map(
              (op) =>
                `({queryId:"${revision}-${op}",operationName:"${op}",metadata:{featureSwitches:["required_flag"]}});`,
            ).join(""),
        );
      }
      assert.equal(url.origin, "https://x.com");
      assert.equal(init?.credentials, "include");
      assert(new Headers(init?.headers).get("x-client-transaction-id"));
      return responder(url, init);
    },
  );
  const client = new XClient();
  await client.init();
  await client.discover();
  return {
    client,
    calls,
    storage,
    setRevision: (s: string) => {
      revision = s;
    },
    removeSeed: () => {
      noSeed = true;
    },
  };
}

test("modern discovery signs follower requests and refreshes a 404 exactly once", async (t) => {
  let attempts = 0;
  const f = await fixture(t, () =>
    ++attempts === 1
      ? new Response("", { status: 404 })
      : Response.json(page()),
  );
  f.setRevision("updated");
  const result = await f.client.page("1", "followers", "saved-cursor");
  assert.equal(result.kind, "page");
  if (result.kind === "page") {
    assert.equal(result.accounts[0].verified, false);
    assert.equal(result.accounts[0].followers, 0);
  }
  const api = f.calls.filter((c) => c.url.host === "x.com");
  assert.equal(api.length, 2);
  assert(api[0].url.pathname.includes("fresh-Followers"));
  assert(api[1].url.pathname.includes("updated-Followers"));
  for (const c of api)
    assert.equal(
      JSON.parse(c.url.searchParams.get("variables")!).cursor,
      "saved-cursor",
    );
  for (const c of f.calls.filter((c) => c.url.host === "abs.twimg.com")) {
    assert.equal(c.init?.credentials, "omit");
    assert.equal(c.init?.headers, undefined);
  }
});
test("persistent 404 names the failed operation and stops after two reads", async (t) => {
  const f = await fixture(t, () => new Response("", { status: 404 }));
  await assert.rejects(
    f.client.page("1", "followers", null),
    /Followers .*still returned 404/,
  );
  assert.equal(f.calls.filter((c) => c.url.host === "x.com").length, 2);
});
test("403 and rate limits are not retried as stale queries", async (t) => {
  let status = 403;
  const f = await fixture(
    t,
    () =>
      new Response("", {
        status,
        headers: { "x-rate-limit-reset": "2000000000" },
      }),
  );
  await assert.rejects(f.client.page("1", "followers", null), /denied/);
  status = 429;
  await assert.rejects(
    f.client.page("1", "followers", null),
    (e: any) => e.code === "rate_limited" && e.retryAt === 2000000002000,
  );
  assert.equal(f.calls.filter((c) => c.url.host === "x.com").length, 2);
});
test("missing signing data prevents API dispatch and session readiness", async (t) => {
  const f = await fixture(t, () => Response.json({}));
  f.removeSeed();
  f.calls.length = 0;
  await assert.rejects(
    f.client.discover(true),
    /expected 4 animation frames, found 0/,
  );
  await assert.rejects(
    f.client.session(),
    /expected 4 animation frames, found 0/,
  );
  assert.equal(
    f.calls.length,
    0,
    "missing page ingredients need no asset or API requests",
  );
});
test("pausing during a failed read prevents rediscovery and retry", async (t) => {
  const f = await fixture(t, () => {
    f.client.cancelRead();
    return new Response("", { status: 404 });
  });
  const assetsBefore = f.calls.filter(
    (c) => c.url.host === "abs.twimg.com",
  ).length;
  await assert.rejects(f.client.page("1", "followers", null), /paused/);
  assert.equal(f.calls.filter((c) => c.url.host === "x.com").length, 1);
  assert.equal(
    f.calls.filter((c) => c.url.host === "abs.twimg.com").length,
    assetsBefore,
  );
});
test("operation-specific observed features win over unrelated requests", async (t) => {
  const f = await fixture(t, () => Response.json(page()));
  for (const [op, flag] of [
    ["Followers", true],
    ["Following", false],
  ] as const) {
    f.client.observe({
      url: `https://x.com/i/api/graphql/observed-${op}/${op}?features=${encodeURIComponent(JSON.stringify({ required_flag: flag }))}`,
      method: "GET",
      requestHeaders: [],
    } as unknown as chrome.webRequest.OnBeforeSendHeadersDetails);
  }
  await f.client.page("1", "followers", null);
  const call = f.calls.find((c) => c.url.host === "x.com")!;
  assert.equal(
    JSON.parse(call.url.searchParams.get("features")!).required_flag,
    true,
  );
});
test("separate activity channels require complete evidence and stop on a recent reply", async (t) => {
  let recentReply = true;
  const f = await fixture(t, (u) => {
    const op = u.pathname.split("/").pop();
    if (op === "UserByRestId")
      return Response.json({ data: { user: { result: modern() } } });
    if (op === "UserRepliesTimeline")
      return Response.json(
        recentReply ? timeline(new Date().toISOString()) : empty(),
      );
    if (op === "UserRepostsTimeline") return Response.json(empty());
    return Response.json(timeline("2020-01-01"));
  });
  const a = await f.client.inspect("1", "2", policy);
  assert(a.last_activity_ms! > Date.now() - 10000);
  assert(!f.calls.some((c) => c.url.pathname.endsWith("UserRepostsTimeline")));
  recentReply = false;
  const old = await f.client.inspect("1", "2", policy);
  assert(old.coverage_since_ms !== null);
  assert(!f.calls.some((c) => c.url.pathname.includes("friendships")));
});
test("mutation 404 is uncertain and is never replayed", async (t) => {
  const f = await fixture(t, () => new Response("", { status: 404 }));
  f.client.inspect = async () => ({
    ...parseUser(modern()),
    posts: 0,
    checked_at_ms: Date.now(),
  });
  let journal = 0;
  const result = await f.client.remove(
    "1",
    "2",
    policy,
    () => {},
    async () => {
      journal++;
    },
  );
  assert.equal(result.kind, "action");
  if (result.kind === "action") assert.equal(result.status, "uncertain");
  assert.equal(journal, 1);
  const api = f.calls.filter((c) => c.url.host === "x.com");
  assert.equal(api.length, 1);
  assert.equal(api[0].init?.method, "POST");
});
test("pause while reading the CSRF cookie prevents the pending GET", async (t) => {
  const f = await fixture(t, () => Response.json(page()));
  t.mock.method(chrome.cookies, "get", async ({ name }: { name: string }) => {
    if (name === "ct0") f.client.cancelRead();
    return { value: name === "twid" ? "u=1" : "test-csrf" };
  });
  await assert.rejects(f.client.page("1", "followers", null), /paused/);
  assert.equal(f.calls.filter((c) => c.url.host === "x.com").length, 0);
});
test("activity preflight starts at the first page despite captured scrolling cursors", async (t) => {
  const f = await fixture(t, (u) => {
    if (u.pathname.endsWith("UserByRestId"))
      return Response.json({ data: { user: { result: modern() } } });
    const variables = JSON.parse(u.searchParams.get("variables")!);
    return Response.json(
      timeline(variables.cursor ? "2020-01-01" : new Date().toISOString()),
    );
  });
  for (const op of [
    "UserOriginalsTimeline",
    "UserRepliesTimeline",
    "UserRepostsTimeline",
  ]) {
    f.client.observe({
      url: `https://x.com/i/api/graphql/scrolled-${op}/${op}?variables=${encodeURIComponent(JSON.stringify({ userId: "999", cursor: "old-page" }))}`,
      method: "GET",
      requestHeaders: [],
    } as unknown as chrome.webRequest.OnBeforeSendHeadersDetails);
  }
  const account = await f.client.inspect("1", "2", policy);
  assert(account.last_activity_ms! > Date.now() - 10000);
  const activity = f.calls.filter((c) =>
    c.url.pathname.endsWith("UserOriginalsTimeline"),
  );
  assert.equal(activity.length, 1);
  assert.equal(
    JSON.parse(activity[0].url.searchParams.get("variables")!).cursor,
    undefined,
  );
  assert.equal(
    JSON.parse(activity[0].url.searchParams.get("variables")!).userId,
    "2",
  );
});
