import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  parsePage,
  parseRelationship,
  parseUser,
  postingEvidence,
  discoverOperations,
} from "../src/parsers";
import { eligible, parseWork } from "../src/protocol";
const user = (extra: object = {}) => ({
  rest_id: "9007199254740993",
  is_blue_verified: false,
  legacy: {
    screen_name: "person",
    name: "Person",
    verified: false,
    protected: false,
    statuses_count: 10,
  },
  ...extra,
});
test("shared policy fixtures agree with Rust", () => {
  const f = JSON.parse(
    readFileSync("../protocol/fixtures/policy.json", "utf8"),
  );
  for (const c of f.cases)
    assert.equal(
      eligible({ ...f.account, ...c.changes }, f.policy, f.now_ms),
      c.eligible,
      c.name,
    );
});
test("blue verification and unknown fields are preserved", () => {
  assert.equal(parseUser(user({ is_blue_verified: true })).verified, true);
  assert.equal(parseUser(user({ is_blue_verified: undefined })).verified, null);
  assert.equal(parseUser(user({ legacy: { screen_name: "a" } })).posts, null);
  assert.throws(() => parseUser({ id: 9007199254740992 }));
});
test("timeline pagination does not collect tweet authors as followers", () => {
  const json = {
    data: {
      instructions: [
        {
          type: "TimelineAddEntries",
          entries: [
            {
              entryId: "user-1",
              content: { itemContent: { user_results: { result: user() } } },
            },
            {
              entryId: "tweet-2",
              content: {
                itemContent: {
                  tweet_results: {
                    result: {
                      core: {
                        user_results: { result: user({ rest_id: "777" }) },
                      },
                    },
                  },
                },
              },
            },
            {
              entryId: "cursor-bottom",
              content: { cursorType: "Bottom", value: "next" },
            },
          ],
        },
      ],
    },
  };
  const p = parsePage(json);
  assert.equal(p.accounts.length, 1);
  assert.equal(p.next_cursor, "next");
  assert.equal(p.complete, false);
  assert.throws(() => parsePage({ data: {} }));
  assert.throws(() => parsePage({ users: [] }));
});
test("absence needs an actual relationship record", () => {
  assert.throws(() => parseRelationship([], "42"));
  assert.throws(() =>
    parseRelationship([{ id_str: "42", connections: [] }], "42"),
  );
  assert.deepEqual(
    parseRelationship([{ id_str: "42", connections: ["none"] }], "42"),
    { follows_me: false, i_follow: false },
  );
  assert.deepEqual(
    parseRelationship(
      [{ id_str: "42", connections: ["following", "followed_by"] }],
      "42",
    ),
    { follows_me: true, i_follow: true },
  );
});
test("empty and malformed activity stay unknown", () => {
  assert.equal(postingEvidence([], "42", Date.now()).coverage, null);
  assert.equal(
    postingEvidence(
      [{ user: { id_str: "42" }, created_at: "bad" }],
      "42",
      Date.now(),
    ).coverage,
    null,
  );
});
test("pinned posts and conversation ancestors do not set activity", () => {
  const tweet = (id: string, date: string) => ({
    legacy: { user_id_str: id, created_at: date },
  });
  const json = {
    instructions: [
      {
        type: "TimelinePinEntry",
        entry: {
          content: { tweet_results: { result: tweet("42", "2020-01-01") } },
        },
      },
      {
        type: "TimelineAddEntries",
        entries: [
          {
            content: {
              itemContent: {
                tweet_results: { result: tweet("99", "2026-09-10") },
              },
            },
          },
          {
            content: {
              itemContent: {
                tweet_results: { result: tweet("42", "2026-01-01") },
              },
            },
          },
        ],
      },
    ],
  };
  assert.equal(
    postingEvidence(json, "42", Date.parse("2026-06-01")).latest,
    Date.parse("2026-01-01"),
  );
});
test("repost uses the action timestamp rather than the original timestamp", () => {
  const data = [
    {
      user: { id_str: "42" },
      created_at: "2026-09-09",
      retweeted_status: { created_at: "2020-01-01" },
    },
  ];
  assert.equal(
    postingEvidence(data, "42", Date.parse("2026-06-01")).latest,
    Date.parse("2026-09-09"),
  );
});
test("operation discovery reads definitions without executing code", () => {
  const ops = discoverOperations(
    '({queryId:"current-id",operationName:"RemoveFollower",operationType:"mutation",metadata:{featureSwitches:["flag"]}}); evil()',
  );
  assert.equal(ops.RemoveFollower?.id, "current-id");
  assert.equal(ops.RemoveFollower?.features.flag, false);
});
test("protocol rejects arbitrary commands and imprecise numeric IDs", () => {
  assert.throws(() =>
    parseWork({
      command_id: "a".repeat(64),
      owner_id: "1",
      command: { kind: "eval", code: "deleteAll()" },
    }),
  );
  assert.throws(() =>
    parseWork({
      command_id: "a".repeat(64),
      owner_id: "1",
      command: { kind: "remove_follower", target_id: 42 },
    }),
  );
});

test("tombstones and replacement-only responses never prove inactivity", () => {
  const old = {
    content: {
      itemContent: {
        tweet_results: {
          result: { legacy: { user_id_str: "42", created_at: "2020-01-01" } },
        },
      },
    },
  };
  const tombstone = {
    content: {
      itemContent: {
        tweet_results: { result: { __typename: "TweetTombstone" } },
      },
    },
  };
  const cutoff = Date.parse("2026-06-01");
  assert.equal(
    postingEvidence(
      {
        instructions: [
          { type: "TimelineAddEntries", entries: [old, tombstone] },
        ],
      },
      "42",
      cutoff,
    ).coverage,
    null,
  );
  assert.equal(
    postingEvidence(
      { instructions: [{ type: "TimelineReplaceEntry", entry: old }] },
      "42",
      cutoff,
    ).coverage,
    null,
  );
  assert.equal(
    postingEvidence(
      { instructions: [{ type: "TimelineAddEntries", entries: [old] }] },
      "42",
      cutoff,
    ).coverage,
    cutoff,
  );
});

test("unrecognized relationship flags cannot establish absence", () => {
  assert.throws(() =>
    parseRelationship(
      [{ id_str: "42", connections: ["unknown_new_flag"] }],
      "42",
    ),
  );
  assert.throws(() =>
    parseRelationship(
      [{ id_str: "42", connections: ["none", "followed_by"] }],
      "42",
    ),
  );
});
