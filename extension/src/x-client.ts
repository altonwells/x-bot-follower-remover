import {
  OPERATIONS,
  discoverOperations,
  parsePage,
  parseRelationship,
  profileResult,
  postingEvidence,
  type Operation,
  type Template,
} from "./parsers";
import { eligible, type Account, type Policy, type Result } from "./protocol";

export class XError extends Error {
  constructor(
    public code: string,
    message: string,
    public retryAt: number | null = null,
  ) {
    super(message);
  }
}
type Templates = Partial<Record<Operation, Template>>;
export class XClient {
  private bearer: string | null = null;
  private templates: Templates = {};
  private discoveryAt = 0;
  private abort: AbortController | null = null;
  private globalFeatures: Record<string, boolean> = {};
  observe(details: chrome.webRequest.OnBeforeSendHeadersDetails): void {
    if (details.initiator === `chrome-extension://${chrome.runtime.id}`) return;
    const url = new URL(details.url);
    if (url.hostname !== "x.com") return;
    const auth = details.requestHeaders?.find(
      (h) => h.name.toLowerCase() === "authorization",
    )?.value;
    if (auth?.startsWith("Bearer ") && auth !== this.bearer) {
      this.bearer = auth;
      void chrome.storage.session.set({ webBearer: auth });
    }
    const match = url.pathname.match(
      /\/graphql\/([A-Za-z0-9_-]+)\/([A-Za-z]+)$/,
    );
    if (
      match &&
      OPERATIONS.includes(match[2] as Operation) &&
      details.method === "GET"
    ) {
      try {
        const features = JSON.parse(url.searchParams.get("features") ?? "{}");
        for (const [k, v] of Object.entries(features))
          if (typeof v === "boolean") this.globalFeatures[k] = v;
        this.templates[match[2] as Operation] = {
          id: match[1],
          variables: JSON.parse(url.searchParams.get("variables") ?? "{}"),
          features,
          fieldToggles: JSON.parse(
            url.searchParams.get("fieldToggles") ?? "{}",
          ),
        };
        void chrome.storage.session.set({
          templates: this.templates,
          globalFeatures: this.globalFeatures,
        });
      } catch {
        /* an unrelated malformed request is not a usable template */
      }
    }
  }
  async init(): Promise<void> {
    const saved = await chrome.storage.session.get<{
      webBearer?: string;
      templates?: Templates;
      globalFeatures?: Record<string, boolean>;
    }>(["webBearer", "templates", "globalFeatures"]);
    this.bearer ??= saved.webBearer ?? null;
    this.templates = { ...saved.templates, ...this.templates };
    this.globalFeatures = { ...saved.globalFeatures, ...this.globalFeatures };
  }
  cancelRead(): void {
    this.abort?.abort();
  }
  async ownerId(): Promise<string> {
    const cookie = await chrome.cookies.get({
      url: "https://x.com/",
      name: "twid",
    });
    const value = decodeURIComponent(cookie?.value ?? "").replaceAll('"', "");
    const id = value.match(/^u=(\d+)$/)?.[1];
    if (!id)
      throw new XError(
        "login_required",
        "Open x.com and sign in to the account you want to clean.",
      );
    return id;
  }
  async assertOwner(expected: string): Promise<void> {
    if ((await this.ownerId()) !== expected)
      throw new XError(
        "account_changed",
        "The signed-in X account changed. Reconnect before continuing.",
      );
  }
  async discover(force = false): Promise<void> {
    if (!force && Date.now() - this.discoveryAt < 60_000) return;
    const tabs = await chrome.tabs.query({ url: "https://x.com/*" });
    const tab = tabs.find((t) => t.active) ?? tabs[0];
    if (!tab?.id)
      throw new XError("tab_required", "Open an X tab in this Chrome profile.");
    const results = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: () => {
        return [
          ...new Set([
            ...Array.from(document.scripts, (s) => s.src),
            ...performance.getEntriesByType("resource").map((r) => r.name),
          ]),
        ].filter(
          (u) =>
            u.startsWith("https://abs.twimg.com/responsive-web/") &&
            /\.js(?:\?|$)/.test(u),
        );
      },
    });
    const urls = (results[0]?.result ?? []) as string[];
    urls.sort(
      (a, b) => Number(b.includes("/main.")) - Number(a.includes("/main.")),
    );
    for (const url of urls.slice(0, 16)) {
      const parsed = new URL(url);
      if (
        parsed.origin !== "https://abs.twimg.com" ||
        !parsed.pathname.startsWith("/responsive-web/")
      )
        continue;
      try {
        const response = await fetch(url, {
          credentials: "omit",
          signal: AbortSignal.timeout(10_000),
        });
        if (!response.ok) continue;
        const script = await response.text();
        if (script.length > 12_000_000) continue;
        const ops = discoverOperations(script);
        for (const op of OPERATIONS)
          if (ops[op] && (force || !this.templates[op]))
            this.templates[op] = {
              ...ops[op],
              ...this.templates[op],
              id: ops[op]!.id,
            };
        if (!this.bearer) {
          const token = script.match(
            /["'](AAAAAAAAAAAAAAAAAAAAA[A-Za-z0-9%_-]{50,})["']/,
          )?.[1];
          if (token) this.bearer = `Bearer ${decodeURIComponent(token)}`;
        }
        if (
          !force &&
          OPERATIONS.every((op) => this.templates[op]) &&
          this.bearer
        )
          break;
      } catch {
        /* Other loaded chunks or observed browser requests may supply the operation. */
      }
    }
    this.discoveryAt = Date.now();
    await chrome.storage.session.set({
      templates: this.templates,
      webBearer: this.bearer,
      globalFeatures: this.globalFeatures,
    });
  }
  async session(): Promise<Result> {
    const owner = await this.ownerId();
    await this.discover();
    if (!this.bearer)
      throw new XError(
        "discovery_required",
        "Refresh the X tab, then reconnect the extension to capture the current web session.",
      );
    let handle = owner;
    try {
      handle = (await this.profile(owner)).handle || owner;
    } catch (e) {
      if (
        e instanceof XError &&
        ["rate_limited", "login_required", "account_changed"].includes(e.code)
      )
        throw e;
    }
    const capabilities = ["session", "inspect_account", "relationship"];
    if (this.templates.Followers) capabilities.push("followers");
    if (this.templates.Following) capabilities.push("following");
    if (this.templates.RemoveFollower) capabilities.push("remove_follower");
    await this.assertOwner(owner);
    return { kind: "session", owner_id: owner, handle, capabilities };
  }
  private async request(
    path: string,
    query: Record<string, string> = {},
    body?: unknown,
    writeOwner?: string,
    guard?: () => void,
  ): Promise<any> {
    if (!path.startsWith("/i/api/"))
      throw new XError("invalid_request", "Unsupported X path");
    const csrf = await chrome.cookies.get({
      url: "https://x.com/",
      name: "ct0",
    });
    if (!csrf?.value || !this.bearer)
      throw new XError(
        "login_required",
        "Refresh your signed-in X tab and reconnect.",
      );
    const url = new URL(path, "https://x.com");
    for (const [k, v] of Object.entries(query)) url.searchParams.set(k, v);
    if (writeOwner) await this.assertOwner(writeOwner);
    guard?.();
    const controller = new AbortController();
    if (!body) this.abort = controller;
    const timer = setTimeout(() => controller.abort(), 20_000);
    try {
      const response = await fetch(url, {
        method: body ? "POST" : "GET",
        credentials: "include",
        signal: controller.signal,
        headers: {
          authorization: this.bearer,
          "x-csrf-token": csrf.value,
          "x-twitter-auth-type": "OAuth2Session",
          "x-twitter-active-user": "yes",
          "x-twitter-client-language": "en",
          "content-type": "application/json",
        },
        ...(body ? { body: JSON.stringify(body) } : {}),
      });
      if (response.status === 429) {
        const reset = Number(response.headers.get("x-rate-limit-reset"));
        throw new XError(
          "rate_limited",
          "X rate limit reached. Pause before resuming.",
          Number.isFinite(reset) && reset > 0
            ? reset * 1000
            : Date.now() + 60_000,
        );
      }
      if (response.status === 401 || response.status === 403)
        throw new XError(
          "access_denied",
          "X denied this request. Check your login, account restrictions, or private-interface compatibility.",
        );
      if (!response.ok)
        throw new XError(
          `http_${response.status}`,
          `X returned HTTP ${response.status}; this operation may need an adapter update.`,
        );
      const data = await response.json();
      if (Array.isArray(data?.errors) && data.errors.length)
        throw new XError(
          "x_error",
          `X rejected the operation (code ${data.errors[0]?.code ?? "unknown"}).`,
        );
      return data;
    } finally {
      clearTimeout(timer);
      if (this.abort === controller) this.abort = null;
    }
  }
  private template(op: Operation): Template {
    const t = this.templates[op];
    if (!t)
      throw new XError(
        "operation_unavailable",
        `${op} was not discovered. Open the relevant X page, refresh discovery, and reconnect.`,
      );
    return t;
  }
  private async graphql(
    op: Operation,
    variables: Record<string, unknown>,
  ): Promise<any> {
    const t = this.template(op);
    return this.request(`/i/api/graphql/${t.id}/${op}`, {
      variables: JSON.stringify({ ...t.variables, ...variables }),
      features: JSON.stringify({ ...t.features, ...this.globalFeatures }),
      fieldToggles: JSON.stringify(t.fieldToggles),
    });
  }
  async profile(id: string): Promise<Account> {
    let raw;
    if (this.templates.UserByRestId)
      raw = await this.graphql("UserByRestId", {
        userId: id,
        withSafetyModeUserFields: true,
      });
    else
      raw = await this.request("/i/api/1.1/users/show.json", {
        user_id: id,
        include_entities: "false",
      });
    const account = profileResult(raw);
    if (account.id !== id)
      throw new XError("identity_mismatch", "X returned a different profile.");
    return account;
  }
  async relationship(
    id: string,
  ): Promise<{ follows_me: boolean; i_follow: boolean }> {
    return parseRelationship(
      await this.request("/i/api/1.1/friendships/lookup.json", { user_id: id }),
      id,
    );
  }
  async page(
    owner: string,
    list: "followers" | "following",
    cursor: string | null,
  ): Promise<Result> {
    await this.assertOwner(owner);
    const op = list === "followers" ? "Followers" : "Following";
    const raw = await this.graphql(op, {
      userId: owner,
      count: 80,
      includePromotedContent: false,
      withGrokTranslatedBio: false,
      ...(cursor ? { cursor } : { cursor: undefined }),
    });
    await this.assertOwner(owner);
    const page = parsePage(raw);
    for (const a of page.accounts)
      if (list === "followers") a.follows_me = true;
      else a.i_follow = true;
    return { kind: "page", list, ...page };
  }
  async inspect(owner: string, id: string, policy: Policy): Promise<Account> {
    await this.assertOwner(owner);
    const account = await this.profile(id);
    Object.assign(account, await this.relationship(id));
    await this.assertOwner(owner);
    account.checked_at_ms = Date.now();
    if (account.protected !== false) {
      account.activity_note = "Protected or unknown visibility; skipped.";
      return account;
    }
    if (account.posts === 0) {
      account.activity_note = "Fresh profile reports zero current posts.";
      return account;
    }
    const cutoff = Date.now() - policy.inactive_days * 86_400_000;
    try {
      let raw;
      if (this.templates.UserTweetsAndReplies) {
        raw = await this.graphql("UserTweetsAndReplies", {
          userId: id,
          count: 40,
          includePromotedContent: false,
          withCommunity: true,
          withVoice: true,
        });
      } else
        raw = await this.request("/i/api/1.1/statuses/user_timeline.json", {
          user_id: id,
          count: "40",
          include_rts: "true",
          exclude_replies: "false",
          tweet_mode: "extended",
        });
      const evidence = postingEvidence(raw, id, cutoff);
      account.last_activity_ms = evidence.latest;
      account.coverage_since_ms = evidence.coverage;
      account.activity_note = evidence.note;
    } catch (e) {
      if (
        e instanceof XError &&
        ["rate_limited", "account_changed", "login_required"].includes(e.code)
      )
        throw e;
      account.activity_note =
        "Activity lookup failed or was inaccessible; kept unknown.";
    }
    await this.assertOwner(owner);
    return account;
  }
  async remove(
    owner: string,
    id: string,
    policy: Policy,
    guard: () => void,
    beforeWrite: () => Promise<void>,
  ): Promise<Result> {
    const account = await this.inspect(owner, id, policy);
    guard();
    if (account.follows_me === false)
      return {
        kind: "action",
        target_id: id,
        status: "already_absent",
        message: "This account no longer follows you.",
      };
    if (!eligible(account, policy))
      return {
        kind: "action",
        target_id: id,
        status: "skipped",
        message: "Fresh evidence no longer meets the approved cleanup policy.",
      };
    const t = this.template("RemoveFollower");
    await beforeWrite();
    try {
      await this.request(
        `/i/api/graphql/${t.id}/RemoveFollower`,
        {},
        { variables: { target_user_id: id }, queryId: t.id },
        owner,
        guard,
      );
      await this.assertOwner(owner);
      const state = await this.relationship(id);
      await this.assertOwner(owner);
      return {
        kind: "action",
        target_id: id,
        status: state.follows_me ? "uncertain" : "verified_removed",
        message: state.follows_me
          ? "X acknowledged the request, but the follower relationship remains present."
          : "Verified that this account no longer follows you.",
      };
    } catch {
      return {
        kind: "action",
        target_id: id,
        status: "uncertain",
        message:
          "The removal may have been dispatched. Reconcile its outcome before continuing.",
      };
    }
  }
  async reconcile(owner: string, target: string): Promise<Result> {
    await this.assertOwner(owner);
    const relationship = await this.relationship(target);
    await this.assertOwner(owner);
    return {
      kind: "action",
      target_id: target,
      status: relationship.follows_me ? "failed" : "already_absent",
      message: relationship.follows_me
        ? "Follower is present. This old attempt is closed; a new review is needed to try again."
        : "Verified absent after recovery. No removal was replayed.",
    };
  }
  async openProfile(owner: string, id: string): Promise<Result> {
    await this.assertOwner(owner);
    await chrome.tabs.create({ url: `https://x.com/i/user/${id}` });
    return { kind: "opened" };
  }
}
