import type { Snapshot, Row } from "./manager-types";
const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const number = (n: number | null | undefined) =>
  n == null ? "—" : n.toLocaleString();
const compact = (n: number | null | undefined) =>
  n == null
    ? "—"
    : new Intl.NumberFormat(undefined, {
        notation: "compact",
        maximumFractionDigits: 1,
      }).format(n);
const labels: Record<string, string> = {
  all: "All followers",
  keep: "Keeping",
  remove: "Planned removal",
  review: "Needs check",
  queue: "Removal queue",
  removed: "Removed",
};
let state: Snapshot | null = null,
  view = "all",
  page = 0,
  search = "",
  loading = false,
  busy = false,
  connected = false,
  selected: Row | null = null,
  confirmation: "start" | "cancel" | null = null;
let epoch = 0,
  confirmationOwner = "";
let timer: ReturnType<typeof setTimeout> | undefined;
let searchTimer: ReturnType<typeof setTimeout> | undefined;
const text = (id: string, value: string) => {
  $(id).textContent = value;
};
function error(message = "") {
  text("manager-error", message);
  $("manager-error").hidden = !message;
}
function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  className = "",
  content = "",
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  node.className = className;
  node.textContent = content;
  return node;
}
function dialog(id: string) {
  const d = $<HTMLDialogElement>(id);
  if (!d.open) d.showModal();
}
function date(ms: number | null | undefined) {
  if (!ms) return "Not checked";
  return new Date(ms).toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}
function activity(row: Row) {
  const a = row.account;
  if (a.last_activity_ms)
    return `${Math.max(0, Math.floor((Date.now() - a.last_activity_ms) / 86400000))}d`;
  return a.checked_at_ms
    ? a.posts === 0
      ? "No posts"
      : "Unavailable"
    : "Not checked";
}
function badge(row: Row) {
  return el(
    "span",
    `decision ${row.decision}`,
    row.working
      ? "Working"
      : row.queued
        ? "Queued"
        : {
            keep: "Keep",
            remove: "Remove",
            review: "Needs check",
            removed: "Removed",
          }[row.decision],
  );
}
function controls() {
  const ready = connected && !!state && !busy;
  const idle =
    ready &&
    !state!.running &&
    !state!.pending &&
    (state!.paused ||
      !["following", "followers", "inspect"].includes(state!.phase));
  $<HTMLButtonElement>("collect").disabled = !idle;
  $<HTMLButtonElement>("start").disabled = !idle;
  $<HTMLButtonElement>("inspect").disabled = !(idle && state!.scan_complete);
  $<HTMLButtonElement>("pause").disabled = !(
    ready &&
    (state!.running ||
      ["following", "followers", "inspect"].includes(state!.phase))
  );
  $<HTMLButtonElement>("cancel").disabled = !(
    ready &&
    (state!.running ||
      ["following", "followers", "inspect"].includes(state!.phase))
  );
  $<HTMLButtonElement>("settings-open").disabled = !ready;
  $<HTMLButtonElement>("keep-account").disabled =
    !ready || !selected || selected.working;
  text("pause", state?.paused ? "Resume" : "Pause");
}
function render(s: Snapshot) {
  if (state && state.owner_id !== s.owner_id) {
    selected = null;
    confirmation = null;
    $<HTMLDialogElement>("detail-dialog").close();
    $<HTMLDialogElement>("confirm-dialog").close();
  }
  if (!connected) error();
  state = s;
  connected = true;
  page = s.page;
  text("activity-rule", `Inactive >${s.policy.inactive_days}d`);
  text(
    "verified-rule",
    s.policy.skip_verified ? "Keep verified" : "Verified protection off",
  );
  text(
    "following-rule",
    s.policy.skip_following ? "Keep following" : "Following protection off",
  );
  text("account-top", `@${s.handle}`);
  document.querySelectorAll<HTMLElement>("[data-count]").forEach((n) => {
    const value = s.counts[n.dataset.count!] ?? 0;
    n.textContent = compact(value);
    n.title = number(value);
  });
  text("live-status", "Connected");
  $("live-status").classList.add("online");
  const cooldown = s.cooldown_until_ms > Date.now();
  text(
    "run-title",
    s.paused
      ? s.running
        ? "Paused"
        : "Ready"
      : cooldown
        ? "Cooling down"
        : (s.active_kind ?? (s.running ? "Running" : "Ready")),
  );
  $("run-card").hidden =
    !s.running &&
    !s.pending &&
    !["following", "followers", "inspect"].includes(s.phase);
  $("run-card").classList.toggle("paused", s.paused || cooldown);
  text("run-detail", s.active ? `@${s.active.handle}` : "");
  $("run-detail").hidden = !s.active;
  const remaining = Math.max(
    0,
    Math.ceil((s.cooldown_until_ms - Date.now()) / 1000),
  );
  const counts = s.counts;
  const checked = Math.min(s.checked, counts.all);
  text(
    "run-progress",
    cooldown
      ? `${s.cooldown_reason || "X rate limit"} · ${remaining >= 60 ? `${Math.ceil(remaining / 60)}m` : `${remaining}s`} remaining`
      : `${number(checked)} / ${number(counts.all)} checked`,
  );
  const body = $("follower-rows");
  body.replaceChildren();
  for (const row of s.rows) {
    const a = row.account,
      tr = el("tr", row.working ? "working" : "");
    const identity = el("td"),
      wrap = el("div", "account-cell");
    const name = el("div");
    name.append(
      el(
        "div",
        "account-name",
        a.name && a.name !== a.handle ? a.name : `@${a.handle}`,
      ),
    );
    if (a.name && a.name !== a.handle)
      name.append(el("div", "account-handle", `@${a.handle}`));
    wrap.append(name);
    if (a.verified === true) wrap.append(el("span", "cell-tag", "Verified"));
    if (a.i_follow === true) wrap.append(el("span", "cell-tag", "Following"));
    identity.append(wrap);
    const decision = el("td");
    decision.append(badge(row));
    const why = el("td", "reason-column");
    why.append(el("div", "row-reason", row.reason));
    const more = el("td"),
      button = el("button", "details-button", "↗");
    button.setAttribute("aria-label", `Details for @${a.handle}`);
    more.append(button);
    tr.append(
      identity,
      el("td", "numeric", activity(row)),
      el("td", "numeric", compact(a.posts)),
      el("td", "numeric", compact(a.followers)),
      decision,
      why,
      more,
    );
    tr.children[1].setAttribute("title", date(a.last_activity_ms));
    tr.children[2].setAttribute("title", number(a.posts));
    tr.children[3].setAttribute("title", number(a.followers));
    tr.addEventListener("click", () => showDetails(row));
    body.append(tr);
  }
  $("empty").hidden = s.rows.length > 0;
  const empty = $("empty");
  empty.querySelector("h3")!.textContent = s.counts.all
    ? "No accounts in this view"
    : "No followers loaded";
  empty.querySelector("p")!.textContent = s.counts.all
    ? "Try another view or search."
    : "Load followers to begin.";
  text(
    "page-label",
    s.total
      ? `${s.page * 50 + 1}–${Math.min((s.page + 1) * 50, s.total)} of ${number(s.total)} accounts`
      : "0 accounts",
  );
  $<HTMLButtonElement>("previous").disabled = s.page === 0;
  $<HTMLButtonElement>("next").disabled = (s.page + 1) * 50 >= s.total;
  text("sync-label", "Synced just now");
  controls();
  const updated =
    selected && s.rows.find((r) => r.account.id === selected!.account.id);
  if (updated && $<HTMLDialogElement>("detail-dialog").open)
    renderDetails(updated);
}
async function rpc(action: unknown) {
  const response = await chrome.runtime.sendMessage({
    type: "manager",
    owner_id: state?.owner_id ?? "",
    action,
  });
  if (!response?.ok)
    throw Error(
      response?.error ?? "Open remover in your terminal, then connect.",
    );
  return response.data;
}
const loadedVersion = chrome.runtime.getManifest?.().version;
const pageVersion = document.body.dataset.extensionVersion;
const needsReload =
  !!pageVersion && /^\d+\./.test(pageVersion) && loadedVersion !== pageVersion;
$("reload-extension").addEventListener("click", () => chrome.runtime.reload());
async function refresh() {
  if (needsReload) {
    error("Reload the extension to finish updating.");
    $("reload-extension").hidden = false;
    text("live-status", "Update ready");
    controls();
    return;
  }
  if (loading || busy || document.hidden) return;
  loading = true;
  const requestEpoch = epoch;
  try {
    const data = await rpc({
      kind: "snapshot",
      query: { filter: view, page, search },
    });
    if (requestEpoch === epoch) render(data);
  } catch (e) {
    if (requestEpoch !== epoch) return;
    connected = false;
    controls();
    $("live-status").classList.remove("online");
    text("live-status", "Disconnected");
    text(
      "sync-label",
      state ? "Connection lost · showing last snapshot" : "Not connected",
    );
    if (!state)
      error(e instanceof Error ? e.message : "Start remover to connect.");
  } finally {
    loading = false;
    clearTimeout(timer);
    timer = setTimeout(() => void refresh(), requestEpoch === epoch ? 2000 : 0);
  }
}
async function command(action: unknown) {
  if (busy) return;
  busy = true;
  controls();
  error();
  try {
    await rpc(action);
    return true;
  } catch (e) {
    error(e instanceof Error ? e.message : "Action failed.");
    return false;
  } finally {
    busy = false;
    controls();
    void refresh();
  }
}
function setView(next: string) {
  epoch++;
  view = next;
  page = 0;
  text("view-heading", labels[view]);
  document.querySelectorAll<HTMLElement>("#views [data-view]").forEach((n) => {
    n.classList.toggle("active", n.dataset.view === view);
    n.setAttribute("aria-current", n.dataset.view === view ? "page" : "false");
  });
  void refresh();
}
function renderDetails(row: Row) {
  selected = row;
  const a = row.account,
    root = $("detail-content");
  root.replaceChildren();
  const identity = el("div", "detail-identity");
  identity.append(
    el("span", "avatar", (a.name || a.handle).slice(0, 2).toUpperCase()),
  );
  const heading = el("div");
  heading.append(el("h2", "", a.name || a.handle), el("p", "", `@${a.handle}`));
  identity.append(heading);
  root.append(
    identity,
    badge(row),
    el("p", "detail-bio", a.bio || "No bio available."),
  );
  const evidence = el("div", "evidence");
  evidence.append(
    el("strong", "", row.reason),
    el("p", "", a.activity_note || "Activity has not been checked."),
  );
  if (row.retry_at_ms)
    evidence.append(
      el(
        "p",
        "",
        `Retry scheduled: ${new Date(row.retry_at_ms).toLocaleString()}`,
      ),
    );
  root.append(evidence);
  const grid = el("dl", "detail-grid");
  const yes = (v: boolean | null) =>
    v === null ? "Unknown" : v ? "Yes" : "No";
  for (const [label, value] of [
    ["Followers", number(a.followers)],
    ["Following", number(a.following_count)],
    ["Posts", number(a.posts)],
    ["Latest observed activity", date(a.last_activity_ms)],
    ["Activity checked", date(a.checked_at_ms)],
    ["Coverage since", date(a.coverage_since_ms)],
    ["Verified", yes(a.verified)],
    ["You follow", yes(a.i_follow)],
    ["Follows you", yes(a.follows_me)],
    ["Keep exception", a.kept ? "Yes" : "No"],
  ]) {
    const item = el("div");
    item.append(el("dt", "", label), el("dd", "", value));
    grid.append(item);
  }
  root.append(grid);
  $<HTMLAnchorElement>("profile-link").href =
    `https://x.com/i/user/${encodeURIComponent(a.id)}`;
  text("keep-account", a.kept ? "Remove keep exception" : "Keep this account");
  controls();
}
function showDetails(row: Row) {
  renderDetails(row);
  dialog("detail-dialog");
}
function confirm(kind: "start" | "cancel") {
  if (!state) return;
  confirmation = kind;
  confirmationOwner = state.owner_id;
  text(
    "confirm-title",
    kind === "start"
      ? `Start cleanup for @${state.handle}?`
      : "Cancel this job?",
  );
  text(
    "confirm-action",
    kind === "start" ? "Approve cleanup" : "Cancel remaining work",
  );
  const copy = $("confirm-copy");
  copy.replaceChildren();
  if (kind === "start") {
    copy.append(
      el(
        "p",
        "",
        `This approves a full pass through @${state.handle}’s followers. The worker collects the list again, checks activity, and removes matching accounts as it goes.`,
      ),
      el(
        "div",
        "confirm-rule",
        "Remove only: latest visible post is over 30 days old, or no posts on an account at least 30 days old. Always keep verified accounts, people you follow, and accounts marked Keep.",
      ),
      el(
        "p",
        "danger-note",
        "There is no restore-followers action. Close the browser manager any time; the approved local worker continues while Chrome is signed in and your Mac is awake.",
      ),
    );
  } else
    copy.append(
      el(
        "p",
        "",
        "Pending removals will be cancelled. An action already sent to X may still finish. Completed removals cannot be undone.",
      ),
    );
  dialog("confirm-dialog");
}
document
  .querySelectorAll<HTMLElement>("[data-view]")
  .forEach((button) =>
    button.addEventListener("click", () => setView(button.dataset.view!)),
  );
$("search").addEventListener("input", () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => {
    epoch++;
    search = $<HTMLInputElement>("search").value.trim();
    page = 0;
    void refresh();
  }, 250);
});
$("previous").addEventListener("click", () => {
  epoch++;
  page = Math.max(0, page - 1);
  void refresh();
});
$("next").addEventListener("click", () => {
  epoch++;
  page++;
  void refresh();
});
$("collect").addEventListener("click", () => void command({ kind: "collect" }));
$("inspect").addEventListener("click", () => void command({ kind: "inspect" }));
$("pause").addEventListener(
  "click",
  () => void command({ kind: state?.paused ? "resume" : "pause" }),
);
$("start").addEventListener("click", () => confirm("start"));
$("cancel").addEventListener("click", () => confirm("cancel"));
$("confirm-action").addEventListener("click", () => {
  const kind = confirmation;
  $<HTMLDialogElement>("confirm-dialog").close();
  confirmation = null;
  if (confirmationOwner !== state?.owner_id) {
    error("Account changed. Review the cleanup again.");
    return;
  }
  if (kind)
    void command(kind === "start" ? { kind, confirmed: true } : { kind });
});
$("confirm-dismiss").addEventListener("click", () =>
  $<HTMLDialogElement>("confirm-dialog").close(),
);
$("keep-account").addEventListener("click", async () => {
  if (
    selected &&
    (await command({
      kind: "keep",
      target_id: selected.account.id,
      kept: !selected.account.kept,
    }))
  ) {
    $<HTMLDialogElement>("detail-dialog").close();
  }
});
$("connection-open").addEventListener("click", () =>
  dialog("connection-dialog"),
);
document
  .querySelectorAll(".close-dialog")
  .forEach((b) =>
    b.addEventListener("click", () => b.closest("dialog")!.close()),
  );
function settings(policy: Snapshot["policy"]) {
  const form = $<HTMLFormElement>("processing-form");
  for (const key of [
    "delay_seconds",
    "rest_every",
    "rest_seconds",
    "batch_limit",
  ] as const)
    (form.elements.namedItem(key) as HTMLInputElement).value = String(
      policy[key],
    );
  (form.elements.namedItem("keep_awake") as HTMLInputElement).checked =
    policy.keep_awake;
}
$("settings-open").addEventListener("click", () => {
  if (state) {
    settings(state.policy);
    dialog("settings-dialog");
  }
});
$("recommended").addEventListener("click", () => {
  if (state)
    settings({
      ...state.policy,
      delay_seconds: 60,
      rest_every: 20,
      rest_seconds: 300,
      batch_limit: 50,
      keep_awake: false,
    });
});
$("processing-form").addEventListener("submit", (event) => {
  event.preventDefault();
  if (!state) return;
  const form = event.currentTarget as HTMLFormElement;
  const policy = { ...state.policy };
  for (const key of [
    "delay_seconds",
    "rest_every",
    "rest_seconds",
    "batch_limit",
  ] as const)
    policy[key] = Number(
      (form.elements.namedItem(key) as HTMLInputElement).value,
    );
  policy.keep_awake = (
    form.elements.namedItem("keep_awake") as HTMLInputElement
  ).checked;
  void command({ kind: "settings", policy }).then((ok) => {
    if (ok) $<HTMLDialogElement>("settings-dialog").close();
  });
});
document.addEventListener("visibilitychange", () => {
  clearTimeout(timer);
  if (!document.hidden) void refresh();
});
document.addEventListener("keydown", (e) => {
  if (
    e.key === "/" &&
    !document.querySelector("dialog[open]") &&
    !["INPUT", "TEXTAREA"].includes((e.target as HTMLElement).tagName)
  ) {
    e.preventDefault();
    $("search").focus();
  }
});
chrome.storage.onChanged.addListener((changes, area) => {
  if (
    area === "session" &&
    (changes.connectionStatus || changes.accountHandle)
  ) {
    epoch++;
    connected = false;
    state = null;
    selected = null;
    $("follower-rows").replaceChildren();
    $<HTMLDialogElement>("detail-dialog").close();
    $<HTMLDialogElement>("confirm-dialog").close();
    controls();
    void refresh();
  }
});
void refresh();
