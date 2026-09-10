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
  if (heartbeat) clearInterval(heartbeat);
  heartbeat = null;
  const old = socket;
  socket = null;
  old?.close();
  await setStatus("Disconnected — pending work is paused");
}
async function connect() {
  await initialized;
  await disconnect();
  const attempt = generation;
  const settings = await chrome.storage.local.get(["port", "token"]);
  if (
    typeof settings.token !== "string" ||
    !/^[a-f0-9]{64}$/.test(settings.token)
  ) {
    await setStatus("Pairing needed — run forgive-me pair");
    return;
  }
  const port = Number(settings.port);
  if (!Number.isInteger(port) || port < 1024 || port > 65535) {
    await setStatus("Invalid local port");
    return;
  }
  if (attempt !== generation) return;
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
          token: settings.token,
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
    controlEpoch++;
    runner.disconnect();
    if (heartbeat) clearInterval(heartbeat);
    heartbeat = null;
    void setStatus("Disconnected — pending work is paused");
    if (retries++ < 3)
      setTimeout(() => {
        if (attempt === generation) void connect();
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
    const recovered = await runner.recovery();
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
    respond({ status });
    return false;
  }
  if (message.type === "connect") {
    retries = 0;
    void connect()
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
chrome.runtime.onStartup.addListener(() => {
  void connect();
});
void connect();
