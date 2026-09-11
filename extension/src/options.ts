import { validSettings } from "./pairing";
const form = document.querySelector<HTMLFormElement>("#pair")!;
const port = document.querySelector<HTMLInputElement>("#port")!;
const token = document.querySelector<HTMLInputElement>("#token")!;
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const account = document.querySelector<HTMLParagraphElement>("#account")!;
const autoPair = document.querySelector<HTMLButtonElement>("#auto-pair")!;
const stored = await chrome.storage.local.get(["port", "token", "pairingMode"]);
port.value = String(stored.port ?? 47831);
token.value = typeof stored.token === "string" ? stored.token : "";
document.querySelector("#version")!.textContent =
  `v${chrome.runtime.getManifest().version}`;
function showAccount(handle: unknown) {
  const identified =
    document.body.dataset.connected === "true" &&
    typeof handle === "string" &&
    !!handle;
  document.querySelector<HTMLElement>("#account-step")!.dataset.complete =
    String(identified);
  document.querySelector<HTMLElement>("#ready-step")!.dataset.complete =
    String(identified);
  account.textContent = identified
    ? `Connected as @${handle}. Confirm this account in your terminal.`
    : "Open X and sign in. Your account will appear here when the terminal identifies it.";
  if (document.body.dataset.connected === "true")
    document.querySelector("#connection-label")!.textContent = identified
      ? "Ready"
      : "Terminal connected";
}
function showStatus(message: string) {
  const connected = message === "Connected";
  document.body.dataset.connected = String(connected);
  document.querySelector<HTMLElement>("#terminal-step")!.dataset.complete =
    String(connected);
  document.querySelector("#connection-label")!.textContent = connected
    ? "Terminal connected"
    : "Not connected";
  status.textContent = connected ? "Your terminal is connected." : message;
  autoPair.textContent = connected
    ? "Reconnect terminal ↗"
    : "Connect terminal ↗";
  if (!connected) showAccount(null);
}
async function show() {
  const state = await chrome.runtime.sendMessage({ type: "status" });
  showStatus(state?.status ?? "Not connected");
  showAccount(state?.accountHandle);
  return state?.status;
}
async function connect(type: "auto_pair" | "connect", settings?: unknown) {
  autoPair.disabled = true;
  showStatus("Connecting to your terminal…");
  try {
    const result = await chrome.runtime.sendMessage({ type, settings });
    if (!result?.ok)
      showStatus(
        result?.error ?? "Unable to connect. Start remover and retry.",
      );
    else await show();
  } catch {
    status.textContent =
      "Unable to reach the extension worker. Reload this page.";
  } finally {
    autoPair.disabled = false;
  }
}
autoPair.addEventListener("click", () => {
  void connect("auto_pair");
});
form.addEventListener("submit", (event) => {
  event.preventDefault();
  void (async () => {
    const settings = { port: Number(port.value), token: token.value.trim() };
    if (!validSettings(settings)) {
      status.textContent =
        "Enter a valid local port and the 64-character pairing secret.";
      return;
    }
    await connect("connect", settings);
  })().catch(() => {
    status.textContent = "Unable to save the connection settings.";
  });
});
document.querySelector("#disconnect")!.addEventListener("click", () => {
  void chrome.runtime
    .sendMessage({ type: "disconnect" })
    .then(show)
    .catch(() => {
      status.textContent = "Reload the extension to disconnect.";
    });
});
document.querySelector("#open-x")!.addEventListener("click", () => {
  void (async () => {
    const [tab] = await chrome.tabs.query({
      url: "https://x.com/*",
      currentWindow: true,
    });
    if (tab?.id) await chrome.tabs.update(tab.id, { active: true });
    else await chrome.tabs.create({ url: "https://x.com/" });
  })().catch(() => {
    account.textContent = "Open x.com in this Chrome profile and sign in.";
  });
});
document.querySelector("#discover")!.addEventListener("click", (event) => {
  const button = event.currentTarget as HTMLButtonElement;
  button.disabled = true;
  status.textContent = "Reading current X operation definitions…";
  void chrome.runtime
    .sendMessage({ type: "discover" })
    .then((result) => {
      status.textContent = result?.ok
        ? "Discovery refreshed. Select Connect terminal."
        : (result?.error ?? "Discovery unavailable");
    })
    .catch(() => {
      status.textContent = "Discovery failed. Refresh X and retry.";
    })
    .finally(() => {
      button.disabled = false;
    });
});
chrome.storage.onChanged.addListener((changes, area) => {
  if (area !== "session") return;
  if (changes.connectionStatus)
    showStatus(String(changes.connectionStatus.newValue));
  if (changes.accountHandle) showAccount(changes.accountHandle.newValue);
});
// Opening setup must not interrupt an already connected cleanup session.
try {
  if ((await show()) !== "Connected")
    await connect(stored.pairingMode === "manual" ? "connect" : "auto_pair");
} catch {
  status.textContent = "Start remover, then select Connect terminal.";
}
export {};
