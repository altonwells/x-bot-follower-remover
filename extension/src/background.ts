import { nativeSettings, validSettings } from "./pairing";
import { XClient, XError } from "./x-client";
import { ChromeJournal } from "./journal";
import { Runner, safeMessage } from "./runner";
import type { Result } from "./protocol";

const client = new XClient();
const runner = new Runner(client, new ChromeJournal());
let socket: WebSocket | null = null,
  session = "",
  generation = 0,
  heartbeat: ReturnType<typeof setInterval> | null = null;
let lastSeen = 0;
let status = "Not connected";
let retries = 0;
let serial = Promise.resolve();
let controlEpoch = 0;
let accountHandle = "";
let settingsWrites = Promise.resolve();
chrome.webRequest.onBeforeSendHeaders.addListener(
  (details) => {
    client.observe(details);
    return undefined;
  },
  { urls: ["https://x.com/i/api/*"] },
  ["requestHeaders", "extraHeaders"],
);
const initialized = client.init();
async function setStatus(s: string) {
  status = s;
  await chrome.storage.session.set({ connectionStatus: s });
  await chrome.action.setBadgeText({ text: s === "Connected" ? "ON" : "" });
}
function send(value: unknown, expected = session) {
  if (socket?.readyState === WebSocket.OPEN && expected === session)
    socket.send(JSON.stringify(value));
}
async function disconnect() {
  generation++;
  controlEpoch++;
  runner.disconnect();
  session = "";
  accountHandle = "";
  if (heartbeat) clearInterval(heartbeat);
  heartbeat = null;
  const old = socket;
  socket = null;
  old?.close();
  await chrome.storage.session.set({ accountHandle });
  await setStatus("Disconnected — pending work is paused");
}
async function connect(
  mode: "saved" | "auto" | "manual" | "retry" = "saved",
  manual?: unknown,
): Promise<void> {
  const stopping = disconnect();
  const attempt = generation;
  await stopping;
  await initialized;
  if (attempt !== generation) return;
  const stored = await chrome.storage.local.get([
    "port",
    "token",
    "pairingMode",
  ]);
  let settings: unknown =
    mode === "manual" && manual !== undefined ? manual : stored;
  if (
    mode === "auto" ||
    (mode === "saved" && stored.pairingMode !== "manual")
  ) {
    try {
      settings = await nativeSettings(
        (host, request) => chrome.runtime.sendNativeMessage(host, request),
        chrome.runtime.id,
      );
    } catch (error) {
      if (mode === "auto" || !validSettings(settings)) {
        if (attempt === generation)
          await setStatus(
            "Start forgive-me in your terminal, then select Connect automatically.",
          );
        throw error;
      }
    }
  }
  if (attempt !== generation) return;
  if (!validSettings(settings))
    throw Error(
      "Use Connect automatically, or enter valid manual pairing settings.",
    );
  const chosen = {
    port: settings.port,
    token: settings.token,
    pairingMode:
      mode === "manual"
        ? "manual"
        : mode === "auto"
          ? "auto"
          : (stored.pairingMode ?? "auto"),
  };
  // Writes finish in request order. A newer explicit request always wins, even
  // when the previous storage write was already in progress at cancellation.
  const save = async () => {
    if (attempt === generation) await chrome.storage.local.set(chosen);
  };
  settingsWrites = settingsWrites.then(save, save);
  await settingsWrites;
  if (attempt !== generation) return;
  const port = chosen.port;
  const ws = new WebSocket(`ws://127.0.0.1:${port}/bridge`);
  socket = ws;
  const timeout = setTimeout(() => {
    if (socket === ws && !session) ws.close();
  }, 5000);
  ws.onopen = () => {
    if (attempt === generation)
      ws.send(
        JSON.stringify({
          v: 1,
          type: "hello",
          token: chosen.token,
          extension_id: chrome.runtime.id,
        }),
      );
  };
  ws.onerror = () => {
    void setStatus("Start the forgive-me TUI, then Connect");
  };
  ws.onclose = () => {
    clearTimeout(timeout);
    if (socket !== ws) return;
    socket = null;
    session = "";
    accountHandle = "";
    void chrome.storage.session.set({ accountHandle });
    controlEpoch++;
    runner.disconnect();
    if (heartbeat) clearInterval(heartbeat);
    heartbeat = null;
    void setStatus("Disconnected — pending work is paused");
    if (retries++ < 3)
      setTimeout(() => {
        if (attempt === generation) void connect("retry").catch(() => {});
      }, 3000);
  };
  ws.onmessage = (event) => {
    void handle(event, ws).catch((e) => {
      void setStatus(safeMessage(e));
    });
  };
}
async function handle(event: MessageEvent, ws: WebSocket): Promise<void> {
  if (
    ws !== socket ||
    typeof event.data !== "string" ||
    event.data.length > 1_048_576
  )
    return;
  const msg = JSON.parse(event.data);
  lastSeen = Date.now();
  if (msg.type === "welcome") {
    if (msg.v !== 1 || typeof msg.session_id !== "string") {
      ws.close();
      return;
    }
    session = msg.session_id;
    runner.connect();
    retries = 0;
    await setStatus("Connected");
    if (ws !== socket) return;
    const recovered = await runner.recovery();
    if (ws !== socket) return;
    if (recovered)
      send({ type: "recovery", session_id: session, ...recovered });
    heartbeat = setInterval(() => {
      if (Date.now() - lastSeen > 35_000) {
        ws.close();
        return;
      }
      send({ type: "heartbeat", session_id: session });
    }, 10_000);
    return;
  }
  if (!session || msg.session_id !== session) return;
  if (msg.type === "heartbeat") {
    send({ type: "heartbeat", session_id: session });
    return;
  }
  if (msg.type === "pause" || msg.type === "cancel") {
    controlEpoch++;
    runner.pause();
    return;
  }
  if (msg.type === "resume") {
    runner.resume();
    return;
  }
  if (msg.type !== "ack" && (msg.type !== "command" || msg.v !== 1)) return;
  const taskSession = session,
    taskEpoch = controlEpoch;
  serial = serial
    .then(async () => {
      if (taskSession !== session) return;
      if (msg.type === "ack") {
        await runner.ack(msg.command_id);
        return;
      }
      let result: Result;
      try {
        if (taskEpoch !== controlEpoch)
          throw Error("Queued task cancelled before execution");
        result = await runner.execute(msg.work);
      } catch (e) {
        result = {
          kind: "error",
          code: e instanceof XError ? e.code : "worker_error",
          message: safeMessage(e),
          retry_at_ms: e instanceof XError ? e.retryAt : null,
        };
      }
      if (
        taskSession === session &&
        msg.work?.command?.kind === "get_session"
      ) {
        accountHandle = result.kind === "session" ? result.handle : "";
        await chrome.storage.session.set({ accountHandle });
      }
      send(
        {
          type: "result",
          session_id: taskSession,
          command_id: msg.work?.command_id,
          result,
        },
        taskSession,
      );
      // An old generation may finish after reconnect. Its journal remains the authoritative receipt.
      if (taskSession !== session && session) {
        const recovered = await runner.recovery();
        if (recovered)
          send({ type: "recovery", session_id: session, ...recovered });
      }
    })
    .catch((e) => {
      void setStatus(safeMessage(e));
    });
}
chrome.action.onClicked.addListener(() => {
  void chrome.runtime.openOptionsPage();
});
chrome.runtime.onMessage.addListener((message, sender, respond) => {
  if (
    sender.id !== chrome.runtime.id ||
    sender.url !== chrome.runtime.getURL("options.html")
  )
    return false;
  if (message.type === "status") {
    respond({ status, accountHandle });
    return false;
  }
  if (message.type === "connect" || message.type === "auto_pair") {
    retries = 0;
    void connect(
      message.type === "auto_pair" ? "auto" : "manual",
      message.settings,
    )
      .then(() => respond({ ok: true }))
      .catch((e) => respond({ ok: false, error: safeMessage(e) }));
    return true;
  }
  if (message.type === "disconnect") {
    void disconnect().then(() => respond({ ok: true }));
    return true;
  }
  if (message.type === "discover") {
    void initialized
      .then(() => client.discover(true))
      .then(() => respond({ ok: true }))
      .catch((e) => respond({ ok: false, error: safeMessage(e) }));
    return true;
  }
  return false;
});
chrome.runtime.onInstalled.addListener((details) => {
  if (details.reason === "install") void chrome.runtime.openOptionsPage();
});
chrome.runtime.onStartup.addListener(() => {
  void connect().catch(() => {});
});
// Page readiness is a hint to the controller, never a reason to tear down pairing.
chrome.tabs.onUpdated.addListener((_id, change, tab) => {
  if (
    change.status === "complete" &&
    tab.url?.startsWith("https://x.com/") &&
    socket?.readyState === WebSocket.OPEN &&
    !accountHandle &&
    session
  ) {
    send({ type: "x_page_ready", session_id: session });
  }
});
void connect().catch(() => {});
