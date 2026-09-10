const form = document.querySelector<HTMLFormElement>("#pair")!;
const port = document.querySelector<HTMLInputElement>("#port")!;
const token = document.querySelector<HTMLInputElement>("#token")!;
const status = document.querySelector<HTMLParagraphElement>("#status")!;
const stored = await chrome.storage.local.get(["port", "token"]);
port.value = String(stored.port ?? 47831);
token.value = typeof stored.token === "string" ? stored.token : "";
async function show() {
  const state = await chrome.runtime.sendMessage({ type: "status" });
  status.textContent = state?.status ?? "Not connected";
}
form.addEventListener("submit", (event) => {
  event.preventDefault();
  void (async () => {
    if (!/^[a-f0-9]{64}$/.test(token.value.trim())) {
      status.textContent =
        "Paste the 64-character secret printed by forgive-me pair.";
      return;
    }
    await chrome.storage.local.set({
      port: Number(port.value),
      token: token.value.trim(),
    });
    status.textContent = "Connecting…";
    const result = await chrome.runtime.sendMessage({ type: "connect" });
    if (!result?.ok) status.textContent = result?.error ?? "Unable to connect";
    else await show();
  })().catch(() => {
    status.textContent =
      "Unable to reach the extension worker. Reload this page.";
  });
});
document.querySelector("#disconnect")!.addEventListener("click", () => {
  void chrome.runtime.sendMessage({ type: "disconnect" }).then(show);
});
document.querySelector("#discover")!.addEventListener("click", (event) => {
  const button = event.currentTarget as HTMLButtonElement;
  button.disabled = true;
  status.textContent = "Reading current X operation definitions…";
  void chrome.runtime
    .sendMessage({ type: "discover" })
    .then((result) => {
      status.textContent = result?.ok
        ? "Discovery refreshed. Save & connect to refresh TUI readiness."
        : (result?.error ?? "Discovery unavailable");
    })
    .finally(() => (button.disabled = false));
});
chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "session" && changes.connectionStatus)
    status.textContent = String(changes.connectionStatus.newValue);
});
await show();
export {};
