import test from "node:test";
import assert from "node:assert/strict";
import { responseLimit } from "../src/rate-limit";

test("429 respects the latest server deadline and adds a margin", () => {
  const state = responseLimit(
    new Headers({ "x-rate-limit-reset": "1500", "retry-after": "600" }),
    429,
    undefined,
    1_000_000,
  );
  assert.equal(state.until, 1_602_000);
  assert.equal(state.failures, 1);
});
test("missing headers back off without clearing a persisted cooldown", () => {
  const first = responseLimit(new Headers(), 429, undefined, 1_000_000);
  const second = responseLimit(new Headers(), 429, first, 1_100_000);
  assert.equal(second.until, 1_222_000);
  assert.equal(
    responseLimit(new Headers(), 200, second, 1_100_001).until,
    second.until,
  );
  assert.equal(
    responseLimit(new Headers(), 200, second, 1_100_001).failures,
    1,
  );
});
test("successful exhausted endpoint waits for reset; unrelated success does not invent quota", () => {
  assert.equal(
    responseLimit(
      new Headers({
        "x-rate-limit-remaining": "0",
        "x-rate-limit-reset": "2000",
      }),
      200,
      undefined,
      1_000_000,
    ).until,
    2_002_000,
  );
  assert.equal(
    responseLimit(new Headers(), 200, undefined, 1_000_000).until,
    0,
  );
});
