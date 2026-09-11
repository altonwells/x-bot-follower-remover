import type { Account, Policy } from "./protocol";
export interface Row {
  account: Account;
  decision: "keep" | "remove" | "review" | "removed";
  reason: string;
  queued: boolean;
  working: boolean;
  retry_at_ms: number | null;
}
export interface Snapshot {
  owner_id: string;
  handle: string;
  generated_at_ms: number;
  rows: Row[];
  counts: Record<string, number>;
  total: number;
  page: number;
  page_size: number;
  phase: string;
  paused: boolean;
  running: boolean;
  pending: boolean;
  active: { id: string; handle: string; name: string } | null;
  active_kind: string | null;
  message: string;
  policy: Policy & {
    rest_every: number;
    rest_seconds: number;
    keep_awake: boolean;
  };
  cooldown_until_ms: number;
  cooldown_reason: string;
  attempts_this_hour: number;
  checked: number;
  removed_total: number;
  uncertain: number;
  scan_complete: boolean;
}
