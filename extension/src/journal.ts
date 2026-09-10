import type { Result, Work } from "./protocol";
export interface Receipt {
  work: Work;
  state: "accepted" | "dispatched" | "finished";
  result?: Result;
  ack_for?: string;
}
export interface JournalStore {
  get(): Promise<Receipt | null>;
  set(receipt: Receipt | null): Promise<void>;
}
export class ChromeJournal implements JournalStore {
  async get(): Promise<Receipt | null> {
    return (
      (await chrome.storage.local.get<{ receipt?: Receipt }>("receipt"))
        .receipt ?? null
    );
  }
  async set(receipt: Receipt | null): Promise<void> {
    await chrome.storage.local.set({ receipt });
  }
}
export function recoveryResult(receipt: Receipt): Result {
  if (receipt.result) return receipt.result;
  const command = receipt.work.command;
  if (command.kind !== "remove_follower")
    throw new Error("Invalid mutation journal");
  return {
    kind: "action",
    target_id: command.target_id,
    status: receipt.state === "accepted" ? "skipped" : "uncertain",
    message:
      receipt.state === "accepted"
        ? "Stopped before dispatch; no write was attempted."
        : "Worker stopped around dispatch; outcome must be reconciled.",
  };
}
