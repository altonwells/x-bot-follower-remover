import { readSigningPage } from "./page-seed";
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
import {
  assetURL,
  assetPriority,
  scriptReferences,
  signingAsset,
} from "./discovery";
import {
  prepareSigning,
  signingIndices,
  transactionId,
  type SigningKey,
} from "./transaction";

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
  private signing: SigningKey | null = null;
  private readEpoch = 0;
  private assertRead(epoch: number): void {
    if (epoch !== this.readEpoch)
      throw new XError("cancelled", "Task paused before request.");
  }
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
          source: "observed",
          id: match[1],
          variables: JSON.parse(url.searchParams.get("variables") ?? "{}"),
          features,
          fieldToggles: JSON.parse(
            url.searchParams.get("fieldToggles") ?? "{}",
          ),
        };
        void chrome.storage.session.set({
          adapterRevision: 2,
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
      adapterRevision?: number;
      globalFeatures?: Record<string, boolean>;
    }>(["webBearer", "templates", "globalFeatures", "adapterRevision"]);
    this.bearer ??= saved.webBearer ?? null;
    this.templates = {
      ...(saved.adapterRevision === 2 ? saved.templates : {}),
      ...this.templates,
    };
    this.globalFeatures = { ...saved.globalFeatures, ...this.globalFeatures };
  }
  cancelRead(): void {
    this.readEpoch++;
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
  async discover(force = false, required?: Operation): Promise<void> {
    if (
      !force &&
      Date.now() - this.discoveryAt < 60_000 &&
      this.signing &&
      (!required || this.templates[required])
    )
      return;
    const epoch = this.readEpoch;
    const tabs = await chrome.tabs.query({ url: "https://x.com/*" });
    const tab = tabs.find((t) => t.active) ?? tabs[0];
    if (!tab?.id)
      throw new XError("tab_required", "Open an X tab in this Chrome profile.");
    const results = await chrome.scripting.executeScript({
      target: { tabId: tab.id },
      func: readSigningPage,
    });
    const page = results[0]?.result;
    if (!page)
      throw new XError(
        "discovery_required",
        "Could not read X's page; refresh the signed-in X tab.",
      );
    this.assertRead(epoch);
    if (page.error) throw new XError(page.code, page.error, page.retryAt);
    if (force) this.signing = null;
    let signingFailure = !page.key
      ? "verification key missing"
      : page.frames.length !== 4
        ? `expected 4 animation frames, found ${page.frames.length}`
        : "signing script not found";
    let loaded = 0;
    const queue = new Set<string>(
      [
        ...page.scripts,
        ...scriptReferences(
          page.inline,
          "https://abs.twimg.com/responsive-web/client-web/main.js",
        ),
      ].filter((u) => assetURL(u)),
    );
    if (!this.signing && (!page.key || page.frames.length !== 4)) queue.clear();
    const seen = new Set<string>();
    const deadline = Date.now() + 20_000;
    while (queue.size && seen.size < 64 && Date.now() < deadline) {
      if (epoch !== this.readEpoch)
        throw new XError("cancelled", "Discovery paused.");
      const batch = [...queue]
        .sort((a, b) => assetPriority(a) - assetPriority(b))
        .slice(0, Math.min(4, 64 - seen.size));
      for (const url of batch) {
        queue.delete(url);
        seen.add(url);
      }
      const scripts = await Promise.all(
        batch.map(async (url) => {
          try {
            const response = await fetch(url, {
              credentials: "omit",
              signal: AbortSignal.timeout(5_000),
            });
            if (!response.ok) return null;
            const script = await response.text();
            return script.length <= 12_000_000 ? { url, script } : null;
          } catch {
            return null;
          }
        }),
      );
      if (epoch !== this.readEpoch)
        throw new XError("cancelled", "Discovery paused.");
      for (const asset of scripts) {
        if (!asset) continue;
        loaded++;
        const { url, script } = asset;
        for (const ref of scriptReferences(script, url))
          if (!seen.has(ref) && queue.size < 1000) queue.add(ref);
        const ops = discoverOperations(script);
        for (const op of OPERATIONS) {
          const old = this.templates[op];
          if (ops[op] && old?.source !== "observed")
            this.templates[op] = { ...ops[op], source: "bundle" };
        }
        if (!this.signing && signingAsset(url)) {
          try {
            this.signing = prepareSigning(page, signingIndices(script));
          } catch (error) {
            signingFailure =
              error instanceof Error ? error.message : "invalid signing data";
          }
        }
        if (!this.bearer) {
          const token = script.match(
            /["'](AAAAAAAAAAAAAAAAAAAAA[A-Za-z0-9%_-]{50,})["']/,
          )?.[1];
          if (token) this.bearer = `Bearer ${decodeURIComponent(token)}`;
        }
      }
      const ready =
        ["Followers", "Following", "UserByRestId", "RemoveFollower"].every(
          (op) => this.templates[op as Operation],
        ) &&
        ([
          "UserOriginalsTimeline",
          "UserRepliesTimeline",
          "UserRepostsTimeline",
        ].every((op) => this.templates[op as Operation]) ||
          this.templates.UserTweetsAndReplies);
      if (
        this.signing &&
        this.bearer &&
        (required ? this.templates[required] : ready)
      )
        break;
    }
    if (!this.signing)
      throw new XError(
        "signing_unavailable",
        `Signing setup failed: ${signingFailure}. Source: ${page.source ?? "document"}; key: ${page.key ? "present" : "missing"}; frames: ${page.frames.length}; assets: ${loaded} loaded, ${seen.size - loaded} failed.`,
      );
    this.discoveryAt = Date.now();
    await chrome.storage.session.set({
      adapterRevision: 2,
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
    const handle = (await this.profile(owner)).handle;
    if (!handle)
      throw new XError(
        "identity_unavailable",
        "X did not return the signed-in account's handle.",
      );
    const capabilities = [
      "adapter:2",
      "session",
      "inspect_account",
      "relationship",
    ];
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
    const epoch = this.readEpoch;
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
    if (!this.signing) await this.discover();
    if (!this.signing)
      throw new XError(
        "signing_unavailable",
        "X request signing could not be prepared. Refresh your signed-in X tab, Refresh X discovery, then retry.",
      );
    const tid = await transactionId(
      this.signing,
      body ? "POST" : "GET",
      url.pathname,
    );
    this.assertRead(epoch);
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
          "x-client-transaction-id": tid,
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
          `X returned HTTP ${response.status} for ${path.split("/").pop()}.`,
        );
      let data;
      try {
        data = await response.json();
      } catch {
        throw new XError(
          "invalid_response",
          `${path.split("/").pop()} returned non-JSON data; scan completeness is unknown.`,
        );
      }
      if (epoch !== this.readEpoch)
        throw new XError(
          "cancelled",
          "Task paused before response was accepted.",
        );
      if (
        Array.isArray(data?.errors) &&
        data.errors.some((e: { code?: number }) => e.code === 88)
      )
        throw new XError(
          "rate_limited",
          "X reported an API rate limit. Wait before resuming.",
          Date.now() + 60_000,
        );
      if (Array.isArray(data?.errors) && data.errors.length)
        throw new XError(
          "x_error",
          `${path.split("/").pop()} rejected the operation (code ${data.errors[0]?.code ?? "unknown"}).`,
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
    const epoch = this.readEpoch;
    if (!this.templates[op]) await this.discover(false, op);
    for (let attempt = 0; attempt < 2; attempt++) {
      this.assertRead(epoch);
      const t = this.template(op);
      try {
        return await this.request(`/i/api/graphql/${t.id}/${op}`, {
          variables: JSON.stringify({ ...t.variables, ...variables }),
          features: JSON.stringify(
            t.source === "observed"
              ? { ...this.globalFeatures, ...t.features }
              : { ...t.features, ...this.globalFeatures },
          ),
          fieldToggles: JSON.stringify(t.fieldToggles),
        });
      } catch (e) {
        this.assertRead(epoch);
        if (!(e instanceof XError) || e.code !== "http_404") throw e;
        if (attempt === 1)
          throw new XError(
            "http_404",
            `${op} (${t.id}) still returned 404 after refreshing its query and signing data. Open that X page, refresh discovery, then retry. Saved progress is intact.`,
          );
        delete this.templates[op];
        await this.discover(true, op);
      }
    }
    throw new XError("operation_unavailable", `${op} unavailable`);
  }
  async profile(id: string): Promise<Account> {
    const raw = await this.graphql("UserByRestId", {
      userId: id,
      withSafetyModeUserFields: true,
    });
    const account = profileResult(raw);
    if (account.id !== id)
      throw new XError("identity_mismatch", "X returned a different profile.");
    return account;
  }
  async relationship(
    id: string,
    snapshot?: Account,
  ): Promise<{ follows_me: boolean; i_follow: boolean }> {
    const account = snapshot ?? (await this.profile(id));
    if (
      typeof account.follows_me === "boolean" &&
      typeof account.i_follow === "boolean"
    )
      return { follows_me: account.follows_me, i_follow: account.i_follow };
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
    const epoch = this.readEpoch;
    await this.assertOwner(owner);
    this.assertRead(epoch);
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
    const epoch = this.readEpoch;
    await this.assertOwner(owner);
    this.assertRead(epoch);
    const account = await this.profile(id);
    this.assertRead(epoch);
    Object.assign(account, await this.relationship(id, account));
    await this.assertOwner(owner);
    this.assertRead(epoch);
    account.checked_at_ms = Date.now();
    if (account.protected !== false) {
      account.activity_note = "Protected or unknown visibility; skipped.";
      return account;
    }
    if (
      (policy.skip_verified && account.verified !== false) ||
      (policy.skip_following && account.i_follow !== false)
    ) {
      account.activity_note = "Excluded by verification or following policy.";
      return account;
    }
    if (account.posts === 0) {
      account.activity_note = "Fresh profile reports zero current posts.";
      return account;
    }
    const cutoff = Date.now() - policy.inactive_days * 86_400_000;
    const modern = [
      "UserOriginalsTimeline",
      "UserRepliesTimeline",
      "UserRepostsTimeline",
    ] as const;
    if (!modern.every((op) => this.templates[op])) await this.discover();
    if (modern.every((op) => this.templates[op])) {
      const evidence = [];
      for (const op of modern) {
        this.assertRead(epoch);
        const raw = await this.graphql(op, {
          userId: id,
          cursor: undefined,
          count: 40,
          includePromotedContent: false,
          withCommunity: true,
          withVoice: true,
        });
        const part = postingEvidence(
          raw,
          id,
          cutoff,
          op === "UserRepostsTimeline",
        );
        evidence.push(part);
        // One recent action is sufficient to protect the account.
        if (part.latest !== null && part.latest > cutoff) break;
      }
      const times = evidence.flatMap((e) =>
        e.latest === null ? [] : [e.latest],
      );
      account.last_activity_ms = times.length ? Math.max(...times) : null;
      const recent =
        account.last_activity_ms !== null && account.last_activity_ms > cutoff;
      account.coverage_since_ms =
        recent ||
        (evidence.length === 3 && evidence.every((e) => e.coverage !== null))
          ? cutoff
          : null;
      account.activity_note = recent
        ? "Recent post, reply or repost observed."
        : account.coverage_since_ms !== null
          ? "Posts, replies and reposts all predate the cutoff."
          : "One or more activity timelines lack adequate evidence; kept unknown.";
    } else if (this.templates.UserTweetsAndReplies) {
      this.assertRead(epoch);
      const raw = await this.graphql("UserTweetsAndReplies", {
        userId: id,
        cursor: undefined,
        count: 40,
        includePromotedContent: false,
        withCommunity: true,
        withVoice: true,
      });
      const evidence = postingEvidence(raw, id, cutoff);
      account.last_activity_ms = evidence.latest;
      account.coverage_since_ms = evidence.coverage;
      account.activity_note = evidence.note;
    } else {
      throw new XError(
        "operation_unavailable",
        "Activity operations are missing. Open a profile's Posts, Replies and Reposts tabs on X, refresh discovery, then retry.",
      );
    }
    await this.assertOwner(owner);
    this.assertRead(epoch);
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
