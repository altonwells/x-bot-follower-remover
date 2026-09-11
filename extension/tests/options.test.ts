import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import vm from "node:vm";
import { createRequire } from "node:module";
const { parseHTML } = createRequire(import.meta.url)("linkedom");
import { transform } from "esbuild";
import { validSettings } from "../src/pairing";
const source = fs
  .readFileSync("src/options.ts", "utf8")
  .replace(/^import .*;$/gm, "")
  .replace(/^export \{\};$/gm, "");
const code = (
  await transform(`(async () => {${source}\n})()`, {
    loader: "ts",
    target: "es2022",
  })
).code;
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
async function harness(connected = true) {
  const { document, window } = parseHTML(
    fs.readFileSync("options.html", "utf8"),
  );
  let state = {
    status: connected ? "Connected" : "Not connected",
    accountHandle: connected ? "demo_account" : "",
  };
  let changed: (changes: any, area: string) => void = () => {};
  const calls: any[] = [];
  await vm.runInNewContext(code, {
    document,
    validSettings,
    chrome: {
      storage: {
        local: { get: async () => ({}) },
        onChanged: {
          addListener: (fn: typeof changed) => {
            changed = fn;
          },
        },
      },
      runtime: {
        getManifest: () => ({ version: "0.3.0" }),
        sendMessage: async (message: any) => {
          calls.push(message);
          if (message.type === "status") return state;
          if (message.type === "disconnect") {
            state = { status: "Not connected", accountHandle: "" };
            return { ok: true };
          }
          return { ok: false, error: "Start remover in your terminal first" };
        },
      },
      tabs: {
        query: async () => [{ id: 7 }],
        update: async (...args: any[]) => {
          calls.push({ tab: args });
        },
      },
    },
  });
  return { document, window, calls, changed };
}
test("opening an already paired page shows readiness without reconnecting", async () => {
  const h = await harness();
  assert.deepEqual(
    h.calls.map((c) => c.type),
    ["status"],
  );
  assert.equal(
    h.document.querySelector("#connection-label")!.textContent,
    "Ready",
  );
  assert.match(
    h.document.querySelector("#account")!.textContent!,
    /@demo_account/,
  );
  assert.equal(
    h.document.querySelector("#ready-step")!.getAttribute("data-complete"),
    "true",
  );
});
test("failed automatic pairing presents a usable retry, with no fake success", async () => {
  const h = await harness(false);
  assert.match(
    h.document.querySelector("#status")!.textContent!,
    /Start remover/,
  );
  assert.equal(
    h.document.querySelector("#auto-pair")!.hasAttribute("disabled"),
    false,
  );
  assert.equal(
    h.document.querySelector("#connection-label")!.textContent,
    "Not connected",
  );
  h.document
    .querySelector("#auto-pair")!
    .dispatchEvent(new h.window.Event("click"));
  await flush();
  assert.equal(h.calls.filter((c) => c.type === "auto_pair").length, 2);
});
test("account loss and disconnect clear the visible ready state", async () => {
  const h = await harness();
  h.changed({ accountHandle: { newValue: "" } }, "session");
  assert.equal(
    h.document.querySelector("#connection-label")!.textContent,
    "Terminal connected",
  );
  h.document
    .querySelector("#disconnect")!
    .dispatchEvent(new h.window.Event("click"));
  await flush();
  assert.equal(h.document.body.dataset.connected, "false");
  assert.equal(
    h.document.querySelector("#ready-step")!.getAttribute("data-complete"),
    "false",
  );
  assert.doesNotMatch(
    h.document.querySelector("#account")!.textContent!,
    /demo_account/,
  );
});
test("Open X focuses an existing tab without starting cleanup", async () => {
  const h = await harness();
  h.document
    .querySelector("#open-x")!
    .dispatchEvent(new h.window.Event("click"));
  await flush();
  assert.equal(h.calls.at(-1).tab[0], 7);
  assert.equal(h.calls.filter((c) => c.type && c.type !== "status").length, 0);
});
