export interface Policy {
  simple_cleanup?: boolean;
  keep_awake?: boolean;
  inactive_days: number;
  skip_verified: boolean;
  skip_following: boolean;
  include_zero_posts: boolean;
  sparse_old_max_posts?: number;
  delay_seconds: number;
  batch_limit: number;
}
export interface Account {
  id: string;
  handle: string;
  name: string;
  bio: string;
  followers: number | null;
  following_count: number | null;
  posts: number | null;
  created_at_ms?: number | null;
  verified: boolean | null;
  protected: boolean | null;
  follows_me: boolean | null;
  i_follow: boolean | null;
  last_activity_ms: number | null;
  coverage_since_ms: number | null;
  checked_at_ms: number | null;
  observed_at_ms: number;
  activity_note: string;
  kept: boolean;
}
export type Command =
  | { kind: "get_session" }
  | {
      kind: "scan_page";
      list: "followers" | "following";
      cursor: string | null;
    }
  | { kind: "inspect_account"; target_id: string; policy: Policy }
  | {
      kind: "remove_follower";
      target_id: string;
      batch_id: string;
      policy: Policy;
      deadline_ms: number;
      approved_account?: Account;
    }
  | { kind: "reconcile"; target_id: string; original_command_id: string }
  | { kind: "open_profile"; target_id: string };
export interface Work {
  command_id: string;
  owner_id: string;
  command: Command;
}
export type Result =
  | {
      kind: "session";
      owner_id: string;
      handle: string;
      capabilities: string[];
    }
  | {
      kind: "page";
      list: string;
      accounts: Account[];
      next_cursor: string | null;
      complete: boolean;
    }
  | { kind: "account"; account: Account }
  | {
      kind: "action";
      target_id: string;
      status:
        | "verified_removed"
        | "already_absent"
        | "skipped"
        | "failed"
        | "uncertain";
      message: string;
    }
  | {
      kind: "deferred";
      target_id: string;
      code: string;
      message: string;
      retry_at_ms: number;
    }
  | { kind: "opened" }
  | {
      kind: "error";
      code: string;
      message: string;
      retry_at_ms: number | null;
    };
export function parseWork(value: unknown): Work {
  if (!value || typeof value !== "object") throw new Error("Invalid command");
  const w = value as Work;
  if (
    typeof w.command_id !== "string" ||
    !/^[a-f0-9]{64}$/.test(w.command_id) ||
    typeof w.owner_id !== "string" ||
    !w.command
  )
    throw new Error("Invalid command identity");
  const c = w.command;
  if (
    ![
      "get_session",
      "scan_page",
      "inspect_account",
      "remove_follower",
      "reconcile",
      "open_profile",
    ].includes(c.kind)
  )
    throw new Error("Unsupported command");
  if (c.kind !== "get_session" && !/^\d+$/.test(w.owner_id))
    throw new Error("Owner ID required");
  if (
    "target_id" in c &&
    (typeof c.target_id !== "string" || !/^\d+$/.test(c.target_id))
  )
    throw new Error("Target ID required");
  if (
    c.kind === "scan_page" &&
    (!["followers", "following"].includes(c.list) ||
      (c.cursor !== null && typeof c.cursor !== "string"))
  )
    throw new Error("Invalid page command");
  if (c.kind === "inspect_account" || c.kind === "remove_follower") {
    const p = c.policy;
    if (
      !p ||
      (p.simple_cleanup !== undefined && typeof p.simple_cleanup !== "boolean") ||
      (p.simple_cleanup === true && (p.inactive_days !== 30 || !p.skip_verified || !p.skip_following)) ||
      !Number.isInteger(p.inactive_days) ||
      p.inactive_days < 1 ||
      p.inactive_days > 3650 ||
      (p.sparse_old_max_posts !== undefined &&
        (!Number.isInteger(p.sparse_old_max_posts) ||
          p.sparse_old_max_posts < 0 ||
          p.sparse_old_max_posts > 100)) ||
      !["skip_verified", "skip_following", "include_zero_posts"].every(
        (k) => typeof p[k as keyof Policy] === "boolean",
      )
    )
      throw new Error("Invalid cleanup policy");
  }
  if (
    (c.kind === "inspect_account" || c.kind === "remove_follower") &&
    (!Number.isInteger(c.policy.delay_seconds) ||
      c.policy.delay_seconds < 5 ||
      c.policy.delay_seconds > 300 ||
      !Number.isInteger(c.policy.batch_limit) ||
      c.policy.batch_limit < 1 ||
      c.policy.batch_limit > 500)
  )
    throw new Error("Invalid pacing policy");
  if (
    c.kind === "remove_follower" &&
    (!Number.isFinite(c.deadline_ms) || typeof c.batch_id !== "string")
  )
    throw new Error("Invalid removal command");
  if (c.kind === "reconcile" && typeof c.original_command_id !== "string")
    throw new Error("Invalid reconciliation command");
  return w;
}
export function eligible(a: Account, p: Policy, now = Date.now(), approved = false): boolean {
  if (approved && p.simple_cleanup && typeof a.checked_at_ms === "number" && a.checked_at_ms <= now) now = a.checked_at_ms;
  if (
    a.kept ||
    a.follows_me !== true ||
    a.protected !== false ||
    (p.skip_following && a.i_follow !== false) ||
    (p.skip_verified && a.verified !== false)
  )
    return false;
  if (
    typeof a.checked_at_ms !== "number" ||
    !Number.isFinite(a.checked_at_ms) ||
    a.checked_at_ms > now + 60_000 ||
    now - a.checked_at_ms > 86_400_000
  )
    return false;
  const cutoff = now - p.inactive_days * 86_400_000;
  if (a.last_activity_ms !== null && a.last_activity_ms > cutoff) return false;
  if (p.simple_cleanup) {
    if (a.posts === 0) return typeof a.created_at_ms === "number" && a.created_at_ms > 0 && a.created_at_ms <= cutoff;
    return typeof a.last_activity_ms === "number" && Number.isFinite(a.last_activity_ms) && a.last_activity_ms > 0 && a.last_activity_ms <= cutoff;
  }
  if (p.include_zero_posts && a.posts === 0) return true;
  if (a.coverage_since_ms !== null && a.coverage_since_ms <= cutoff)
    return true;
  return (
    (p.sparse_old_max_posts ?? 0) > 0 &&
    a.posts !== null &&
    a.posts > 0 &&
    a.posts <= p.sparse_old_max_posts! &&
    a.last_activity_ms !== null &&
    a.last_activity_ms > 0 &&
    a.last_activity_ms <= cutoff
  );
}
