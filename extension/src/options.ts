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
  `forgive-me ${chrome.runtime.getManifest().version} · Account data stays on your computer.`;
function showAccount(handle: unknown) {
  account.textContent =
    typeof handle === "string" && handle
      ? `Connected as @${handle}. Confirm this account in your terminal.`
      : "Sign in to X. The terminal will identify your account after the page loads.";
}
async function show() {
  const state = await chrome.runtime.sendMessage({ type: "status" });
  status.textContent = state?.status ?? "Not connected";
  showAccount(state?.accountHandle);
  return state?.status;
}
async function connect(type: "auto_pair" | "connect", settings?: unknown) {
  autoPair.disabled = true;
  status.textContent = "Connecting to your terminal…";
  try {
    const result = await chrome.runtime.sendMessage({ type, settings });
    if (!result?.ok)
      status.textContent =
        result?.error ?? "Unable to connect. Start forgive-me and retry.";
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
        ? "Discovery refreshed. Select Connect automatically."
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
    status.textContent = String(changes.connectionStatus.newValue);
  if (changes.accountHandle) showAccount(changes.accountHandle.newValue);
});
// Opening setup must not interrupt an already connected cleanup session.
try {
  if ((await show()) !== "Connected")
    await connect(stored.pairingMode === "manual" ? "connect" : "auto_pair");
} catch {
  status.textContent = "Start forgive-me, then select Connect automatically.";
}
export {};
