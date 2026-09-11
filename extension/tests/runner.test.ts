import { test } from "node:test";
import assert from "node:assert/strict";
import { Runner, type Client } from "../src/runner";
import {
  type Receipt,
  type JournalStore,
  recoveryResult,
} from "../src/journal";
import type { Work, Result } from "../src/protocol";
const policy = {
  inactive_days: 90,
  skip_verified: true,
  skip_following: true,
  include_zero_posts: true,
  delay_seconds: 10,
  batch_limit: 50,
};
const work = (id = "a"): Work => ({
  command_id: id.repeat(64),
  owner_id: "1",
  command: {
    kind: "remove_follower",
    target_id: "2",
    batch_id: "batch",
    policy,
    deadline_ms: Date.now() + 120_000,
  },
});
class Journal implements JournalStore {
  receipt: Receipt | null = null;
  async get() {
    return this.receipt ? structuredClone(this.receipt) : null;
  }
  async set(r: Receipt | null) {
    this.receipt = r ? structuredClone(r) : null;
  }
}
function setup() {
  const journal = new Journal();
  let writes = 0;
  const client: Client = {
    session: async () => ({
      kind: "session",
      owner_id: "1",
      handle: "owner",
      capabilities: [],
    }),
    page: async () => {
      throw Error("unused");
    },
    inspect: async () => {
      throw Error("unused");
    },
    cancelRead: () => {},
    openProfile: async () => ({ kind: "opened" }),
    reconcile: async () => ({
      kind: "action",
      target_id: "2",
      status: "already_absent",
      message: "absent",
    }),
    remove: async (_o, _i, _p, guard, before) => {
      await before();
      guard();
      writes++;
      return {
        kind: "action",
        target_id: "2",
        status: "verified_removed",
        message: "done",
      };
    },
  };
  const runner = new Runner(client, journal);
  runner.connect();
  return { runner, journal, client, writes: () => writes };
}
test("duplicate command returns stored receipt without another write", async () => {
  const s = setup();
  await s.runner.execute(work());
  await s.runner.execute(work());
  assert.equal(s.writes(), 1);
  await s.runner.ack(work().command_id);
  await assert.rejects(s.runner.execute(work()), /replayed/);
  assert.equal(s.writes(), 1);
});
test("worker restart recovers a dispatched action as uncertain", async () => {
  const s = setup();
  s.journal.receipt = { work: work(), state: "dispatched" };
  const r = new Runner(s.client, s.journal);
  r.connect();
  assert.equal((await r.recovery())?.result.kind, "action");
  assert.equal((recoveryResult(s.journal.receipt) as any).status, "uncertain");
  await assert.rejects(r.execute(work("b")), /outstanding/);
  assert.equal(s.writes(), 0);
});
test("pause during preflight prevents dispatch", async () => {
  const s = setup();
  s.client.remove = async (_o, _i, _p, guard, before) => {
    s.runner.pause();
    guard();
    await before();
    throw Error("should not reach");
  };
  const result = await s.runner.execute(work());
  assert.equal((result as any).status, "skipped");
  assert.equal(s.writes(), 0);
});
test("disconnect and reconnect cannot revive an old command", async () => {
  const s = setup();
  s.client.remove = async (_o, _i, _p, guard, before) => {
    s.runner.disconnect();
    s.runner.connect();
    guard();
    await before();
    throw Error("should not reach");
  };
  const result = await s.runner.execute(work());
  assert.equal((result as any).status, "skipped");
  assert.equal(s.writes(), 0);
});
test("disconnect while awaiting journal never adopts a new generation", async () => {
  const s = setup();
  const get = s.journal.get.bind(s.journal);
  let first = true;
  s.journal.get = async () => {
    if (first) {
      first = false;
      s.runner.disconnect();
      s.runner.connect();
    }
    return get();
  };
  await assert.rejects(s.runner.execute(work()), /stopped/);
  assert.equal(s.writes(), 0);
});
test("only one execution can pass the asynchronous journal boundary", async () => {
  const s = setup();
  const first = s.runner.execute(work());
  await assert.rejects(s.runner.execute(work("b")), /still running/);
  await first;
  assert.equal(s.writes(), 1);
});
test("uncertain receipt survives ack and clears only after reconciliation acknowledgement", async () => {
  const s = setup();
  s.journal.receipt = { work: work(), state: "dispatched" };
  await s.runner.ack(work().command_id);
  assert.ok(s.journal.receipt);
  const w: Work = {
    command_id: "c".repeat(64),
    owner_id: "1",
    command: {
      kind: "reconcile",
      target_id: "2",
      original_command_id: work().command_id,
    },
  };
  const result = await s.runner.execute(w);
  assert.equal((result as any).status, "already_absent");
  assert.ok(s.journal.receipt);
  await s.runner.ack(w.command_id);
  assert.equal(s.journal.receipt, null);
  assert.equal(s.writes(), 0);
});
test("rate limit during preflight fails and stops the batch, rather than skipping every account", async () => {
  const s = setup();
  s.client.remove = async () => {
    throw Error("X rate limit reached");
  };
  const r = await s.runner.execute(work());
  assert.equal((r as any).status, "failed");
  assert.equal(s.writes(), 0);
});
test("crash-like exception after dispatch is not reported as a clean failure", async () => {
  const s = setup();
  s.client.remove = async (_o, _i, _p, _guard, before) => {
    await before();
    throw Error("connection lost");
  };
  const r = await s.runner.execute(work());
  assert.equal((r as any).status, "uncertain");
  assert.equal((await s.runner.recovery())?.result.kind, "action");
});

test("a queued command cannot clear a pause", async () => {
  const s = setup();
  s.runner.pause();
  await assert.rejects(s.runner.execute(work()), /stopped/);
  assert.equal(s.writes(), 0);
  s.runner.resume();
  await s.runner.execute(work());
  assert.equal(s.writes(), 1);
});
test("pause then resume cannot revive preflight already in progress", async () => {
  const s = setup();
  s.client.remove = async (_o, _i, _p, guard, before) => {
    s.runner.pause();
    s.runner.resume();
    guard();
    await before();
    throw Error("unreachable");
  };
  const result = await s.runner.execute(work());
  assert.equal((result as any).status, "skipped");
  assert.equal(s.writes(), 0);
});

test("pre-dispatch rate limit retains a deferred receipt until acknowledged", async () => {
  const s = setup();
  const retryAt = Date.now() + 120_000;
  s.client.remove = async () => {
    throw Object.assign(new Error("budget exhausted"), {
      code: "rate_limited",
      retryAt,
    });
  };
  const result = await s.runner.execute(work());
  assert.deepEqual(result, {
    kind: "deferred",
    target_id: "2",
    code: "rate_limited",
    message: "budget exhausted",
    retry_at_ms: retryAt,
  });
  assert.equal(s.writes(), 0);
  const restart = new Runner(s.client, s.journal);
  assert.deepEqual((await restart.recovery())?.result, result);
  await restart.ack(work().command_id);
  assert.equal(s.journal.receipt, null);
});

test("a rate-limit error after dispatch remains uncertain and cannot be retried", async () => {
  const s = setup();
  s.client.remove = async (_o, _i, _p, _g, before) => {
    await before();
    throw Object.assign(new Error("429"), {
      code: "rate_limited",
      retryAt: Date.now() + 60000,
    });
  };
  const result = await s.runner.execute(work());
  assert.equal(result.kind, "action");
  assert.equal((result as any).status, "uncertain");
  await s.runner.ack(work().command_id);
  assert(s.journal.receipt);
});

test("explicit durable acknowledgement transfers uncertain recovery to the paired controller", async () => {
  const s=setup();s.journal.receipt={work:work(),state:"dispatched"};
  await s.runner.ack(work().command_id);assert(s.journal.receipt);
  await s.runner.ack(work().command_id,true);assert.equal(s.journal.receipt,null);
  await assert.rejects(s.runner.execute(work()),/replayed/);
});
