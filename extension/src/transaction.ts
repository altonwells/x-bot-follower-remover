// Protocol port informed by twscrape/xclid.py (MIT) and XClientTransaction (MIT).
// See THIRD_PARTY_NOTICES.md. Seeds stay in Chrome; every request gets a fresh ID.
export interface SigningSeed {
  key: string;
  frames: string[];
}
export interface SigningKey {
  bytes: number[];
  animation: string;
}

export function signingIndices(script: string): number[] {
  return [...script.matchAll(/\(\w+\[(\d{1,2})\],\s*16\)/g)].map((m) =>
    Number(m[1]),
  );
}
function cubic(curve: number[], time: number): number {
  const [x1, y1, x2, y2] = curve;
  const at = (a: number, b: number, t: number) =>
    3 * a * (1 - t) ** 2 * t + 3 * b * (1 - t) * t ** 2 + t ** 3;
  if (time <= 0) return time * (x1 > 0 ? y1 / x1 : x2 > 0 ? y2 / x2 : 0);
  if (time >= 1)
    return (
      1 +
      (time - 1) *
        (x2 < 1 ? (y2 - 1) / (x2 - 1) : x1 < 1 ? (y1 - 1) / (x1 - 1) : 0)
    );
  let lo = 0,
    hi = 1,
    mid = 0;
  for (let i = 0; i < 60; i++) {
    mid = (lo + hi) / 2;
    const x = at(x1, x2, mid);
    if (Math.abs(time - x) < 0.00001) break;
    if (x < time) lo = mid;
    else hi = mid;
  }
  return at(y1, y2, mid);
}
export function animationKey(frame: number[], time: number): string {
  if (
    frame.length !== 11 ||
    frame.some((v) => !Number.isInteger(v) || v < 0 || v > 255)
  )
    throw Error("X signing animation format changed");
  const scale = (v: number, min: number, max: number) =>
    (v * (max - min)) / 255 + min;
  const curve = frame
    .slice(7)
    .map((v, i) => Number(scale(v, i % 2 ? -1 : 0, 1).toFixed(2)));
  const progress = cubic(curve, time);
  const colors = frame
    .slice(0, 3)
    .map((v, i) =>
      Math.round(
        Math.max(0, Math.min(255, v + (frame[i + 3] - v) * progress)),
      ).toString(16),
    );
  const rotation =
    (Math.floor(scale(frame[6], 60, 360)) * progress * Math.PI) / 180;
  const matrix = [
    Math.cos(rotation),
    -Math.sin(rotation),
    Math.sin(rotation),
    Math.cos(rotation),
  ].map((v) => Math.abs(Number(v.toFixed(2))).toString(16));
  return [...colors, ...matrix, "0", "0"].join("").replace(/[.-]/g, "");
}
export function prepareSigning(
  seed: SigningSeed,
  indices: number[],
): SigningKey {
  const bytes = [...atob(seed.key)].map((c) => c.charCodeAt(0));
  if (
    bytes.length < 6 ||
    bytes.length > 128 ||
    indices.length < 2 ||
    indices.length > 8 ||
    indices.some((i) => i >= bytes.length) ||
    seed.frames.length !== 4
  )
    throw Error(
      "X signing ingredients unavailable; refresh your signed-in X tab",
    );
  const rows = seed.frames[bytes[5] % 4]
    .slice(9)
    .split("C")
    .map((row) => row.match(/\d+/g)?.map(Number) ?? []);
  const row = rows[bytes[indices[0]] % 16];
  if (!row) throw Error("X signing animation row missing");
  const time =
    (Math.round(
      indices.slice(1).reduce((t, i) => t * (bytes[i] % 16), 1) / 10,
    ) *
      10) /
    4096;
  return { bytes, animation: animationKey(row, time) };
}
export async function transactionId(
  key: SigningKey,
  method: string,
  path: string,
): Promise<string> {
  const seconds = Math.floor(Date.now() / 1000) - 1682924400;
  const hash = new Uint8Array(
    await crypto.subtle.digest(
      "SHA-256",
      new TextEncoder().encode(
        `${method.toUpperCase()}!${path}!${seconds}obfiowerehiring${key.animation}`,
      ),
    ),
  );
  const mask = crypto.getRandomValues(new Uint8Array(1))[0];
  const bytes = [
    ...key.bytes,
    ...[0, 1, 2, 3].map((i) => (seconds >>> (i * 8)) & 255),
    ...hash.slice(0, 16),
    3,
  ];
  return btoa(String.fromCharCode(mask, ...bytes.map((v) => v ^ mask))).replace(
    /=+$/,
    "",
  );
}
