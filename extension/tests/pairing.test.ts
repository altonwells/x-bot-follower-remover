import assert from "node:assert/strict";
import test from "node:test";
import { nativeSettings, validSettings, NATIVE_HOST } from "../src/pairing";
const id = "a".repeat(32);
const token = "b".repeat(64);
test("native pairing requests only the named host and accepts bound settings", async () => {
  const result = await nativeSettings(async (host, request) => {
    assert.equal(host, NATIVE_HOST);
    assert.deepEqual(request, { type: "pair", v: 1 });
    return {
      ok: true,
      v: 1,
      extension_id: id,
      port: 47831,
      token,
      extra: "discard",
    };
  }, id);
  assert.deepEqual(result, { port: 47831, token });
});
test("pairing rejects wrong identities, versions, ports, and secrets", async () => {
  const valid = { ok: true, v: 1, extension_id: id, port: 47831, token };
  for (const patch of [
    { extension_id: "c".repeat(32) },
    { v: 2 },
    { port: 0 },
    { port: 65536 },
    { port: 47831.5 },
    { port: "47831" },
    { token: "bad" },
    { ok: false },
  ]) {
    await assert.rejects(
      nativeSettings(async () => ({ ...valid, ...patch }), id),
    );
  }
  for (const value of [null, "string", undefined, []])
    await assert.rejects(nativeSettings(async () => value, id));
});
test("native helper errors remain actionable without returning credentials", async () => {
  await assert.rejects(
    nativeSettings(
      async () => ({
        ok: false,
        error: "Start forgive-me in your terminal first",
      }),
      id,
    ),
    /Start forgive-me/,
  );
  await assert.rejects(
    nativeSettings(async () => {
      throw Error("Native host not found");
    }, id),
    /Native host not found/,
  );
});
test("manual pairing uses the same port and secret bounds", () => {
  assert.ok(validSettings({ port: 1024, token }));
  assert.ok(validSettings({ port: 65535, token }));
  assert.ok(!validSettings({ port: 1023, token }));
  assert.ok(!validSettings({ port: 47831, token: "B".repeat(64) }));
});
