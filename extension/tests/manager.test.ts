import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import vm from "node:vm";
import { createRequire } from "node:module";
import { transform } from "esbuild";
const { parseHTML } = createRequire(import.meta.url)("linkedom");
const source = fs
  .readFileSync("src/manager.ts", "utf8")
  .replace(/^import .*;$/gm, "");
const code = (await transform(source, { loader: "ts", format: "iife" })).code;
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
function data() {
  return {
    owner_id: "1",
    handle: "example",
    rows: [
      {
        account: {
          id: "2",
          name: "<img src=x onerror=bad()>",
          handle: "quiet_account",
          bio: "<script>bad()</script>",
          posts: 2,
          followers: 10,
          following_count: 30,
          last_activity_ms: Date.now() - 60 * 86400000,
          checked_at_ms: Date.now(),
          coverage_since_ms: Date.now() - 31 * 86400000,
          verified: false,
          i_follow: false,
          follows_me: true,
          kept: false,
        },
        decision: "remove",
        reason: "No posts in 30 days",
        working: false,
        queued: false,
        retry_at_ms: null,
      },
    ],
    counts: { all: 1, keep: 0, remove: 1, review: 0, removed: 0, queue: 0 },
    total: 1,
    page: 0,
    page_size: 50,
    phase: "done",
    paused: true,
    running: false,
    pending: false,
    active: null,
    active_kind: null,
    message: "Ready",
    policy: {
      simple_cleanup: true,
      inactive_days: 30,
      skip_verified: true,
      skip_following: true,
      delay_seconds: 60,
      batch_limit: 50,
      rest_every: 20,
      rest_seconds: 300,
      keep_awake: false,
    },
    cooldown_until_ms: 0,
    cooldown_reason: "",
    attempts_this_hour: 0,
    checked: 1,
    removed_total: 0,
    uncertain: 0,
    scan_complete: true,
  };
}
async function harness(
  respond?: (message: any) => Promise<any>,
  loadedVersion?: string,
) {
  const { document, window } = parseHTML(
    fs.readFileSync("options.html", "utf8"),
  );
  if (loadedVersion) document.body.dataset.extensionVersion = "0.4.1";
  const calls: any[] = [];
  let changed: any;
  let scheduled: (() => void) | undefined;
  document.querySelectorAll("dialog").forEach((d: any) => {
    d.showModal = () => {
      d.open = true;
      d.setAttribute("open", "");
    };
    d.close = () => {
      d.open = false;
      d.removeAttribute("open");
    };
  });
  vm.runInNewContext(code, {
    document,
    console,
    Date,
    setTimeout: (fn: () => void) => {
      scheduled = fn;
      return 0;
    },
    clearTimeout() {},
    chrome: {
      runtime: {
        getManifest: () => ({ version: loadedVersion ?? "0.4.1" }),
        reload: () => calls.push({ type: "reload" }),
        sendMessage: async (message: any) => {
          calls.push(message);
          if (respond) return respond(message);
          return {
            ok: true,
            data:
              message.action.kind === "snapshot" ? data() : { accepted: true },
          };
        },
      },
      storage: {
        onChanged: {
          addListener(fn: any) {
            changed = fn;
          },
        },
      },
    },
  });
  await flush();
  return { document, window, calls, changed, tick: () => scheduled?.() };
}
test("manager opening reads saved state only and renders untrusted profiles as text", async () => {
  const h = await harness();
  assert.deepEqual(
    h.calls.map((c) => c.action.kind),
    ["snapshot"],
  );
  assert.equal(h.document.querySelectorAll("#follower-rows tr").length, 1);
  assert.equal(h.document.querySelectorAll("#follower-rows img").length, 0);
  assert.match(
    h.document.querySelector("#follower-rows").textContent,
    /img src=x/,
  );
  h.document.querySelector(".details-button").click();
  assert.match(
    h.document.querySelector("#detail-content").textContent,
    /<script>/,
  );
  assert.equal(h.document.querySelectorAll("#detail-content script").length, 0);
});
test("cleanup is sent only after confirmation and names the current account", async () => {
  const h = await harness();
  h.document.querySelector("#start").click();
  assert.match(
    h.document.querySelector("#confirm-title").textContent,
    /@example/,
  );
  assert.equal(h.calls.filter((c) => c.action.kind === "start").length, 0);
  h.document.querySelector("#confirm-action").click();
  await flush();
  const start = h.calls.find((c) => c.action.kind === "start");
  assert.equal(start.owner_id, "1");
  assert.equal(start.action.confirmed, true);
});
test("filters read server decisions and keep changes target the selected account", async () => {
  const h = await harness();
  h.document.querySelector('[data-view="remove"]').click();
  await flush();
  assert.equal(h.calls.at(-1).action.query.filter, "remove");
  h.document.querySelector(".details-button").click();
  h.document.querySelector("#keep-account").click();
  await flush();
  assert.deepEqual(
    JSON.parse(
      JSON.stringify(h.calls.find((c) => c.action.kind === "keep").action),
    ),
    { kind: "keep", target_id: "2", kept: true },
  );
});
test("connection changes clear profiles and dismiss a pending approval", async () => {
  const h = await harness();
  h.document.querySelector("#start").click();
  h.changed({ accountHandle: { newValue: "" } }, "session");
  assert.equal(h.document.querySelectorAll("#follower-rows tr").length, 0);
  assert.equal(h.document.querySelector("#confirm-dialog").open, false);
  assert.equal(h.document.querySelector("#start").disabled, true);
});

test("a filter changed during polling discards the old result and requests the latest view", async () => {
  let resolve!: (value: any) => void;
  let count = 0;
  const h = await harness(async (m) => {
    count++;
    if (count === 2)
      return new Promise((r) => {
        resolve = r;
      });
    return {
      ok: true,
      data: {
        ...data(),
        rows: m.action.query.filter === "keep" ? [] : data().rows,
      },
    };
  });
  h.tick();
  await flush();
  h.document.querySelector('[data-view="keep"]').click();
  resolve({ ok: true, data: data() });
  await flush();
  h.tick();
  await flush();
  assert.equal(h.calls.at(-1).action.query.filter, "keep");
  assert.equal(h.document.querySelectorAll("#follower-rows tr").length, 0);
  assert.equal(
    h.document.querySelector("#view-heading").textContent,
    "Keeping",
  );
});

test("an old running extension presents Reload before any manager requests", async () => {
  const h = await harness(undefined, "0.3.2");
  assert.equal(h.calls.length, 0);
  assert.match(
    h.document.querySelector("#manager-error").textContent,
    /Reload the extension/,
  );
  assert.equal(h.document.querySelector("#reload-extension").hidden, false);
  h.document.querySelector("#reload-extension").click();
  assert.equal(h.calls[0].type, "reload");
});
