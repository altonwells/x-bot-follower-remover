import test from "node:test";
import assert from "node:assert/strict";
import vm from "node:vm";
import { createRequire } from "node:module";
const { DOMParser } = createRequire(import.meta.url)("linkedom");
import { readSigningPage } from "../src/page-seed";
import { XClient } from "../src/x-client";
import { OPERATIONS } from "../src/parsers";
import { prepareSigning, signingIndices } from "../src/transaction";
import { scriptReferences } from "../src/discovery";

const key = btoa(
  String.fromCharCode(...Array.from({ length: 48 }, (_, i) => i)),
);
const path =
  "M0 0 0 0 " + Array(16).fill("10 20 30 40 50 60 70 80 90 100 110").join("C");
const shell = `<html><head><meta name="twitter-site-verification" content="${key}"><script src="https://abs.twimg.com/x-web/assets/app-main.js"></script></head><body>READY</body></html>`;
const html = shell.replace(
  "READY",
  Array.from(
    { length: 4 },
    (_, i) =>
      `<svg id="loading-x-anim-${i}"><g><path d="M0 0"/><path d="${path}"/></g></svg>`,
  ).join(""),
);
function execute(fn: Function, doc: string, fetcher: typeof fetch) {
  return vm.runInNewContext(`(${fn.toString()})()`, {
    document: new DOMParser().parseFromString(doc, "text/html"),
    DOMParser,
    performance: { getEntriesByType: () => [] },
    fetch: fetcher,
    URL,
    AbortSignal,
    Date,
  });
}

test("serialized Chrome collector restores removed loading frames from original HTML", async () => {
  assert.throws(
    () => prepareSigning({ key, frames: [] }, [2, 12, 14, 7]),
    /animation frames/,
  );
  let requests = 0;
  const result = await execute(readSigningPage, shell, async (url, init) => {
    requests++;
    assert.equal(url, "https://x.com/home");
    assert.equal(init?.credentials, "include");
    assert.equal(init?.redirect, "error");
    return new Response(html);
  });
  assert.equal(requests, 1);
  assert.equal(result.source, "html");
  assert.equal(result.frames.length, 4);
  assert(prepareSigning(result, [2, 12, 14, 7]).animation.length > 0);
  const live = await execute(readSigningPage, html, async () => {
    throw Error("must not fetch");
  });
  assert.equal(live.source, "document");
});

test("HTML fallback preserves rate limits and does not claim a login page is signing-ready", async () => {
  const limited = await execute(
    readSigningPage,
    shell,
    async () => new Response("", { status: 429 }),
  );
  assert.equal(limited.code, "rate_limited");
  assert(limited.retryAt > Date.now());
  const loggedOut = await execute(
    readSigningPage,
    shell,
    async () => new Response("<html><body>Sign in</body></html>"),
  );
  assert.equal(loggedOut.key, "");
  assert.equal(loggedOut.frames.length, 0);
  // Fresh HTML must not borrow the previous document's key or SVGs.
  assert.throws(
    () => prepareSigning(loggedOut, [2, 12, 14, 7]),
    /verification key missing/,
  );
});

test("bare signing chunk names resolve within the X asset origin", () => {
  const urls = scriptReferences(
    'import("sign.o-abc.js"); import("../sign.o-next.js"); import("design.o-no.js"); import("https://evil.test/sign.o-bad.js")',
    "https://abs.twimg.com/x-web/assets/app.js",
  );
  assert(urls.includes("https://abs.twimg.com/x-web/assets/sign.o-abc.js"));
  assert(urls.includes("https://abs.twimg.com/x-web/sign.o-next.js"));
  assert.equal(urls.length, 2);
  assert.deepEqual(
    signingIndices("($[2],16)*(_bytes[12],16)*(a[14],16)*(a[7],16)"),
    [2, 12, 14, 7],
  );
});

test("real collector and adapter identify account after live SVG removal and bare chunk discovery", async (t) => {
  const previous = globalThis.chrome;
  t.after(() => {
    globalThis.chrome = previous;
  });
  const calls: string[] = [];
  const fetcher: typeof fetch = async (input, init) => {
    const url = String(input);
    calls.push(url);
    if (url === "https://x.com/home") return new Response(html);
    if (url.endsWith("app-main.js"))
      return new Response(
        'import("sign.o-fixture.js");' +
          OPERATIONS.map(
            (op) => `({queryId:"test-${op}",operationName:"${op}"});`,
          ).join(""),
      );
    if (url.endsWith("sign.o-fixture.js"))
      return new Response("($[2],16)*(a[12],16)*(a[14],16)*(a[7],16)");
    assert(url.endsWith("/UserByRestId") || url.includes("/UserByRestId?"));
    assert(new Headers(init?.headers).get("x-client-transaction-id"));
    return Response.json({
      data: {
        user: { result: { rest_id: "1", core: { screen_name: "example" } } },
      },
    });
  };
  Object.assign(globalThis, {
    chrome: {
      runtime: { id: "test" },
      cookies: {
        get: async ({ name }: any) => ({
          value: name === "twid" ? "u=1" : "csrf",
        }),
      },
      tabs: { query: async () => [{ id: 1, active: true }] },
      scripting: {
        executeScript: async ({ func }: any) => [
          { result: await execute(func, shell, fetcher) },
        ],
      },
      storage: {
        session: {
          get: async () => ({ webBearer: "Bearer fixture" }),
          set: async () => {},
        },
      },
    },
  });
  t.mock.method(globalThis, "fetch", fetcher);
  const client = new XClient();
  await client.init();
  const result = await client.session();
  assert.equal(result.kind, "session");
  if (result.kind === "session") {
    assert.equal(result.owner_id, "1");
    assert.equal(result.handle, "example");
  }
  assert(calls.includes("https://x.com/home"));
  assert(
    calls.includes("https://abs.twimg.com/x-web/assets/sign.o-fixture.js"),
  );
});
