import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  animationKey,
  transactionId,
  signingIndices,
  prepareSigning,
} from "../src/transaction";
import { assetURL, scriptReferences, signingAsset } from "../src/discovery";

test("signing animation matches the pinned independent Python reference", () => {
  const { vectors } = JSON.parse(
    readFileSync("tests/fixtures/signing.json", "utf8"),
  );
  for (const v of vectors)
    assert.equal(animationKey(v.frame, v.time), v.expected);
});
test("transaction signature binds method, path and timestamp with fresh random masking", async () => {
  const key = {
    bytes: Array.from({ length: 48 }, (_, i) => i),
    animation: "abcdef",
  };
  const decode = (s: string) => {
    const b = [...atob(s)].map((c) => c.charCodeAt(0));
    return b.slice(1).map((v) => v ^ b[0]);
  };
  const value = await transactionId(key, "GET", "/i/api/graphql/id/Followers");
  const raw = decode(value);
  assert.deepEqual(raw.slice(0, 48), key.bytes);
  const time = raw.slice(48, 52).reduce((n, v, i) => n + v * 2 ** (8 * i), 0);
  const digest = new Uint8Array(
    await crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(
        `GET!/i/api/graphql/id/Followers!${time}obfiowerehiringabcdef`,
      ),
    ),
  );
  assert.deepEqual(raw.slice(52, 68), [...digest.slice(0, 16)]);
  assert.equal(raw[68], 3);
  const other = decode(
    await transactionId(key, "POST", "/i/api/graphql/id/RemoveFollower"),
  );
  assert.notDeepEqual(other.slice(52, 68), raw.slice(52, 68));
});
test("discovery follows only X asset paths across Vite and webpack formats", () => {
  const base = "https://abs.twimg.com/x-web/assets/main.js";
  const urls = scriptReferences(
    'import("./sign.o-abc.js"); import("./followers.js"); import("https://evil.test/stolen.js");{100:"ondemand.s"}+{100:"0123456789abcdef"}',
    base,
  );
  assert(urls.includes("https://abs.twimg.com/x-web/assets/sign.o-abc.js"));
  assert(
    urls.includes(
      "https://abs.twimg.com/responsive-web/client-web/ondemand.s.0123456789abcdefa.js",
    ),
  );
  assert.equal(urls.length, 3);
  assert.equal(assetURL("https://abs.twimg.com.evil.test/x-web/a.js"), null);
  assert.equal(assetURL("https://abs.twimg.com/unrelated/a.js"), null);
  assert(signingAsset(urls[0]));
  assert(!signingAsset("https://abs.twimg.com/x-web/design.o-a.js"));
  assert.deepEqual(signingIndices("(a[2],16)*(longer[42], 16)"), [2, 42]);
  assert.throws(() =>
    prepareSigning({ key: btoa("short"), frames: [] }, [2, 42]),
  );
});
