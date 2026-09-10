import { test } from "node:test";
import assert from "node:assert/strict";
import { XClient } from "../src/x-client";
import type { Account, Policy } from "../src/protocol";
const policy: Policy = {
  inactive_days: 90,
  skip_verified: true,
  skip_following: true,
  include_zero_posts: true,
  delay_seconds: 10,
  batch_limit: 50,
};
test("reconciliation refuses an account switch during the relationship read", async () => {
  const c = new XClient();
  let owner = "1";
  c.ownerId = async () => owner;
  c.relationship = async () => {
    owner = "9";
    return { follows_me: false, i_follow: false };
  };
  await assert.rejects(c.reconcile("1", "2"), /account changed/);
});
test("zero-post and protected early inspection returns check owner again", async () => {
  for (const protectedAccount of [true, false]) {
    const c = new XClient();
    let owner = "1";
    c.ownerId = async () => owner;
    c.profile = async () =>
      ({ id: "2", protected: protectedAccount, posts: 0 }) as Account;
    c.relationship = async () => {
      owner = "9";
      return { follows_me: true, i_follow: false };
    };
    await assert.rejects(c.inspect("1", "2", policy), /account changed/);
  }
});
