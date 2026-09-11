import { parseWork, type Work, type Result } from "./protocol";
import { recoveryResult, type JournalStore, type Receipt } from "./journal";

export interface Client {
  session(): Promise<Result>;
  page(
    owner: string,
    list: "followers" | "following",
    cursor: string | null,
  ): Promise<Result>;
  inspect(owner: string, id: string, p: any): Promise<any>;
  remove(
    owner: string,
    id: string,
    p: any,
    guard: () => void,
    beforeWrite: () => Promise<void>,
  ): Promise<Result>;
  reconcile(owner: string, id: string): Promise<Result>;
  openProfile(owner: string, id: string): Promise<Result>;
  cancelRead(): void;
}
export class Runner {
  private generation = 0;
  private active = false;
  private connected = false;
  private paused = false;
  private recentlyDone = new Set<string>();
  constructor(
    private client: Client,
    private journal: JournalStore,
  ) {}
  connect(): void {
    this.generation++;
    this.connected = true;
    this.paused = false;
  }
  disconnect(): void {
    this.generation++;
    this.connected = false;
    this.paused = true;
    this.client.cancelRead();
  }
  pause(): void {
    this.generation++;
    this.paused = true;
    this.client.cancelRead();
  }
  resume(): void {
    this.paused = false;
  }
  async recovery(): Promise<{ work: Work; result: Result } | null> {
    // A live task is responsible for finishing its own receipt. Never infer "not dispatched" while it can still advance.
    if (this.active) return null;
    const receipt = await this.journal.get();
    return receipt
      ? { work: receipt.work, result: recoveryResult(receipt) }
      : null;
  }
  async ack(id: string): Promise<void> {
    const receipt = await this.journal.get();
    if (receipt && (receipt.work.command_id === id || receipt.ack_for === id)) {
      const result = recoveryResult(receipt);
      // Uncertain receipts stay until an explicit read-only reconciliation resolves them.
      if (result.kind === "action" && result.status === "uncertain") return;
      this.recentlyDone.add(id);
      await this.journal.set(null);
    }
    if (this.recentlyDone.size > 100)
      this.recentlyDone.delete(this.recentlyDone.values().next().value!);
  }
  async execute(input: unknown): Promise<Result> {
    const work = parseWork(input);
    const c = work.command;
    if (!this.connected) throw new Error("Controller disconnected");
    if (this.active) throw new Error("Another browser task is still running");
    this.active = true;
    const generation = this.generation;
    try {
      const receipt = await this.journal.get();
      if (receipt?.work.command_id === work.command_id)
        return recoveryResult(receipt);
      if (this.recentlyDone.has(work.command_id))
        throw new Error("Completed command cannot be replayed");
      if (receipt && c.kind !== "get_session" && c.kind !== "reconcile")
        throw new Error(
          "An outstanding removal receipt must be acknowledged or reconciled first",
        );
      if (
        c.kind === "reconcile" &&
        receipt &&
        (c.original_command_id !== receipt.work.command_id ||
          work.owner_id !== receipt.work.owner_id ||
          (receipt.work.command.kind === "remove_follower" &&
            c.target_id !== receipt.work.command.target_id))
      )
        throw new Error(
          "Reconciliation does not match the outstanding removal",
        );
      const guard = () => {
        if (!this.connected || this.paused || generation !== this.generation)
          throw new Error("Task stopped before write");
        if (c.kind === "remove_follower" && Date.now() > c.deadline_ms)
          throw new Error("Removal dispatch deadline expired");
      };
      switch (c.kind) {
        case "get_session":
          return await this.client.session();
        case "scan_page":
          guard();
          return await this.client.page(work.owner_id, c.list, c.cursor);
        case "inspect_account":
          guard();
          return {
            kind: "account",
            account: await this.client.inspect(
              work.owner_id,
              c.target_id,
              c.policy,
            ),
          };
        case "open_profile":
          return await this.client.openProfile(work.owner_id, c.target_id);
        case "reconcile": {
          const result = await this.client.reconcile(
            work.owner_id,
            c.target_id,
          );
          if (receipt) {
            await this.journal.set({
              ...receipt,
              state: "finished",
              result,
              ack_for: work.command_id,
            });
          }
          return result;
        }
        case "remove_follower": {
          guard();
          let record: Receipt = { work, state: "accepted" };
          await this.journal.set(record);
          let result: Result;
          try {
            result = await this.client.remove(
              work.owner_id,
              c.target_id,
              c.policy,
              guard,
              async () => {
                guard();
                record = { work, state: "dispatched" };
                await this.journal.set(record);
              },
            );
          } catch (e) {
            const message = safeMessage(e);
            const error = e as { code?: string; retryAt?: number };
            if (
              record.state !== "dispatched" &&
              ["rate_limited", "network_unavailable"].includes(
                error?.code ?? "",
              )
            ) {
              result = {
                kind: "deferred",
                target_id: c.target_id,
                code: error.code!,
                message,
                retry_at_ms: Math.max(
                  Date.now() + 1000,
                  Number.isFinite(error.retryAt)
                    ? error.retryAt!
                    : Date.now() + 60_000,
                ),
              };
            } else
              result = {
                kind: "action",
                target_id: c.target_id,
                status:
                  record.state === "dispatched"
                    ? "uncertain"
                    : message.includes("stopped before write") ||
                        message.includes("deadline expired")
                      ? "skipped"
                      : "failed",
                message:
                  record.state === "dispatched"
                    ? "Dispatch outcome uncertain; reconcile before continuing."
                    : message,
              };
          }
          await this.journal.set({ work, state: "finished", result });
          return result;
        }
      }
    } finally {
      this.active = false;
    }
  }
}
export function safeMessage(e: unknown): string {
  return e instanceof Error ? e.message.slice(0, 300) : "Browser task failed";
}
