export const NATIVE_HOST = "com.x_bot_follower_remover.pairing";
export interface PairingSettings {
  port: number;
  token: string;
}
export function validSettings(value: unknown): value is PairingSettings {
  if (!value || typeof value !== "object") return false;
  const s = value as Record<string, unknown>;
  return (
    typeof s.port === "number" &&
    Number.isInteger(s.port) &&
    s.port >= 1024 &&
    s.port <= 65535 &&
    typeof s.token === "string" &&
    /^[a-f0-9]{64}$/.test(s.token)
  );
}
export async function nativeSettings(
  send: (host: string, request: object) => Promise<unknown>,
  id: string,
): Promise<PairingSettings> {
  const value = await send(NATIVE_HOST, { type: "pair", v: 1 });
  if (!value || typeof value !== "object")
    throw Error("Start remover in your terminal, then retry.");
  const response = value as Record<string, unknown>;
  if (response.ok !== true)
    throw Error(
      typeof response.error === "string"
        ? response.error
        : "Automatic pairing failed.",
    );
  if (
    response.v !== 1 ||
    response.extension_id !== id ||
    !validSettings(response)
  )
    throw Error(
      "Invalid automatic pairing response. Update remover and reload the extension.",
    );
  return { port: response.port, token: response.token };
}
