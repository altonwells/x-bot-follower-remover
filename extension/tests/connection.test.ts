// Actual worker connection code, with synthetic Chrome and X services.
import assert from "node:assert/strict";
import test from "node:test";
import fs from "node:fs";
import vm from "node:vm";
import { transform } from "esbuild";
import { nativeSettings, validSettings } from "../src/pairing";
const id = "a".repeat(32);
const token = "b".repeat(64);
const nativeReply = { ok: true, v: 1, port: 47831, token, extension_id: id };
const manual = { port: 49000, token: "c".repeat(64) };
const flush = () => new Promise<void>((resolve) => setImmediate(resolve));
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}
const source = fs
  .readFileSync("src/background.ts", "utf8")
  .replace(/^import .*;$/gm, "");
const code = (await transform(source, { loader: "ts", format: "iife" })).code;
async function harness(
  native: () => Promise<unknown> = async () => nativeReply,
  beforeSave?: () => Promise<void>,
) {
  let listener: any,
    tabUpdated: any,
    nativeCalls = 0;
  let settings: Record<string, unknown> = {};
  const sockets: any[] = [],
    timers: { callback: () => void; ms: number }[] = [];
  class Socket {
    static OPEN = 1;
    readyState = 1;
    onclose?: () => void;
    constructor(public url: string) {
      sockets.push(this);
    }
    close() {
      this.onclose?.();
    }
    send() {}
  }
  vm.runInNewContext(code, {
    Promise,
    Date,
    JSON,
    Error,
    nativeSettings,
    validSettings,
    setInterval: () => 0,
    clearInterval() {},
    setTimeout: (callback: () => void, ms: number) => {
      timers.push({ callback, ms });
      return timers.length;
    },
    clearTimeout() {},
    XError: Error,
    ChromeJournal: class {},
    Runner: class {
      disconnect() {}
      connect() {}
      recovery() {
        return null;
      }
    },
    XClient: class {
      init() {
        return Promise.resolve();
      }
    },
    safeMessage: (e: Error) => e.message,
    WebSocket: Socket,
    chrome: {
      webRequest: { onBeforeSendHeaders: { addListener() {} } },
      tabs: {
        onUpdated: {
          addListener(f: unknown) {
            tabUpdated = f;
          },
        },
      },
      action: { setBadgeText: async () => {}, onClicked: { addListener() {} } },
      storage: {
        session: { set: async () => {} },
        local: {
          get: async () => ({ ...settings }),
          set: async (s: object) => {
            await beforeSave?.();
            settings = { ...settings, ...s };
          },
        },
      },
      runtime: {
        id,
        getURL: (path: string) => path,
        sendNativeMessage: async () => {
          nativeCalls++;
          return native();
        },
        onMessage: {
          addListener(f: unknown) {
            listener = f;
          },
        },
        onInstalled: { addListener() {} },
        onStartup: { addListener() {} },
      },
    },
  });
  await flush();
  return {
    sockets,
    timers,
    settings: () => settings,
    nativeCalls: () => nativeCalls,
    message: (message: object) =>
      new Promise<any>((resolve) =>
        listener(message, { id, url: "options.html" }, resolve),
      ),
    xLoaded: () =>
      tabUpdated(1, { status: "complete" }, { url: "https://x.com/home" }),
  };
}
test("manual pairing supersedes a pending automatic attempt", async () => {
  const pending = deferred<unknown>();
  const h = await harness(() => pending.promise);
  assert.equal(
    (await h.message({ type: "connect", settings: manual })).ok,
    true,
  );
  pending.resolve(nativeReply);
  await flush();
  assert.deepEqual(
    h.sockets.map((s) => s.url),
    ["ws://127.0.0.1:49000/bridge"],
  );
  assert.equal(h.settings().port, 49000);
  assert.equal(h.settings().pairingMode, "manual");
});
test("disconnect then connect starts a new attempt instead of joining a cancelled one", async () => {
  const pending = deferred<unknown>();
  let calls = 0;
  const h = await harness(() =>
    ++calls === 1 ? pending.promise : Promise.resolve(nativeReply),
  );
  await h.message({ type: "disconnect" });
  assert.equal((await h.message({ type: "auto_pair" })).ok, true);
  pending.resolve(nativeReply);
  await flush();
  assert.equal(h.sockets.length, 1);
  assert.equal(h.nativeCalls(), 2);
});
test("retry and X sign-in preserve the manually chosen endpoint", async () => {
  const h = await harness();
  await h.message({ type: "connect", settings: manual });
  h.sockets.at(-1).onclose();
  h.timers.find((t) => t.ms === 3000)!.callback();
  await flush();
  h.xLoaded();
  await flush();
  assert.equal(h.sockets.at(-1).url, "ws://127.0.0.1:49000/bridge");
  assert.equal(h.nativeCalls(), 1);
  assert.equal(h.settings().pairingMode, "manual");
});
test("a new manual request persists after an already-running automatic storage write", async () => {
  const pending = deferred<void>();
  let saves = 0;
  const h = await harness(
    async () => nativeReply,
    () => (++saves === 1 ? pending.promise : Promise.resolve()),
  );
  const result = h.message({ type: "connect", settings: manual });
  await flush();
  pending.resolve();
  await result;
  await flush();
  assert.deepEqual(
    h.sockets.map((s) => s.url),
    ["ws://127.0.0.1:49000/bridge"],
  );
  assert.equal(h.settings().port, 49000);
});
