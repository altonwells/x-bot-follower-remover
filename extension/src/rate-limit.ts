// Persist endpoint cooldowns across MV3 worker restarts. Never wait inside a write.
export interface LimitState {
  until: number;
  failures: number;
}
export function responseLimit(
  headers: Headers,
  status: number,
  previous: LimitState | undefined,
  now = Date.now(),
): LimitState {
  const limited = status === 429;
  const failures = limited ? Math.min((previous?.failures ?? 0) + 1, 8) : 0;
  const reset = Number(headers.get("x-rate-limit-reset")) * 1000;
  const retry = headers.get("retry-after");
  const retryAt =
    retry === null
      ? 0
      : /^\d+(?:\.\d+)?$/.test(retry)
        ? now + Number(retry) * 1000
        : Date.parse(retry);
  const exhausted = headers.get("x-rate-limit-remaining") === "0";
  return {
    failures,
    until: Math.max(
      previous?.until ?? 0,
      limited || exhausted
        ? Math.max(
            Number.isFinite(reset) ? reset : 0,
            Number.isFinite(retryAt) ? retryAt : 0,
            now +
              (limited
                ? Math.min(60_000 * 2 ** (failures - 1), 3_600_000)
                : 60_000),
          ) + 2000
        : 0,
    ),
  };
}
