import type { Account } from "./protocol";
type Obj = Record<string, any>;
const bool = (v: unknown): boolean | null =>
  typeof v === "boolean" ? v : null;
const count = (v: unknown): number | null =>
  typeof v === "number" && Number.isSafeInteger(v) && v >= 0 ? v : null;
const text = (v: unknown): string =>
  typeof v === "string" ? v.slice(0, 4000) : "";
export function parseUser(raw: Obj): Account {
  const u = raw?.result ?? raw;
  const l = u?.legacy ?? u;
  const id = u?.rest_id ?? l?.id_str;
  if (typeof id !== "string" || !/^\d+$/.test(id))
    throw new Error("Profile response missing stable account ID");
  const blue = bool(u.is_blue_verified ?? l.is_blue_verified),
    legacy = bool(u.verification?.verified ?? l.verified);
  const verifiedType = u.verification?.verified_type ?? l.verified_type;
  const verified =
    blue === true ||
    legacy === true ||
    (typeof verifiedType === "string" &&
      !["", "none"].includes(verifiedType.toLowerCase()))
      ? true
      : blue === false && legacy === false
        ? false
        : null;
  const r = u.relationship_perspectives ?? {};
  return {
    id,
    handle: text(u.core?.screen_name ?? l.screen_name),
    name: text(u.core?.name ?? l.name),
    bio: text(u.profile_bio?.description ?? l.description),
    followers: count(u.relationship_counts?.followers ?? l.followers_count),
    following_count: count(u.relationship_counts?.following ?? l.friends_count),
    posts: count(u.tweet_counts?.tweets ?? l.statuses_count),
    created_at_ms: Number.isFinite(Date.parse(u.core?.created_at ?? l.created_at)) ? Date.parse(u.core?.created_at ?? l.created_at) : null,
    verified,
    protected: bool(u.privacy?.protected ?? l.protected),
    follows_me: bool(r.followed_by ?? l.followed_by),
    i_follow: bool(r.following ?? l.following),
    last_activity_ms: null,
    coverage_since_ms: null,
    checked_at_ms: null,
    observed_at_ms: Date.now(),
    activity_note: "Not checked",
    kept: false,
  };
}
export function walk(value: any, visit: (o: Obj) => void, depth = 0): void {
  if (depth > 30 || !value || typeof value !== "object") return;
  if (!Array.isArray(value)) visit(value);
  for (const child of Object.values(value))
    if (child && typeof child === "object") walk(child, visit, depth + 1);
}
export function instructions(json: any): Obj[] {
  let result: Obj[] | undefined;
  walk(json, (o) => {
    if (!result && Array.isArray(o.instructions)) result = o.instructions;
  });
  if (!result)
    throw new Error(
      "Unrecognized timeline response; scan completeness is unknown",
    );
  return result;
}
export function parsePage(json: any): {
  accounts: Account[];
  next_cursor: string | null;
  complete: boolean;
} {
  if (Array.isArray(json?.users)) {
    if (typeof json.next_cursor_str !== "string")
      throw new Error("Missing pagination cursor");
    return {
      accounts: json.users.map(parseUser),
      next_cursor: json.next_cursor_str === "0" ? null : json.next_cursor_str,
      complete: json.next_cursor_str === "0",
    };
  }
  const inst = instructions(json),
    users = new Map<string, Account>();
  let cursor: string | null = null,
    recognized = false,
    terminated = false,
    emptyPage = false;
  for (const i of inst) {
    if (i.type === "TimelineTerminateTimeline" && i.direction === "Bottom") {
      terminated = true;
      recognized = true;
    }
    if (i.type === "TimelineAddEntries" || i.type === "TimelineReplaceEntry") {
      recognized = true;
      if (
        i.type === "TimelineAddEntries" &&
        Array.isArray(i.entries) &&
        i.entries.length === 0
      )
        emptyPage = true;
      for (const e of i.entries ?? [i.entry]) {
        if (!e) continue;
        if (
          e.content?.cursorType === "Bottom" &&
          typeof e.content.value === "string"
        )
          cursor = e.content.value;
        const items = [
          e.content?.itemContent,
          ...(e.content?.items ?? []).map((i: Obj) => i.item?.itemContent),
        ];
        for (const item of items)
          if (item?.user_results?.result) {
            try {
              const a = parseUser(item.user_results.result);
              users.set(a.id, a);
            } catch {
              /* unavailable users are omitted, never synthesized */
            }
          }
      }
    }
  }
  if (!recognized) throw new Error("Timeline did not establish pagination");
  if (cursor === null && !terminated && !(emptyPage && users.size === 0))
    throw new Error(
      "Missing bottom cursor; follower scan completeness is unknown",
    );
  const complete = terminated || cursor === "0" || cursor === null;
  return {
    accounts: [...users.values()],
    next_cursor: complete ? null : cursor,
    complete,
  };
}
export function profileResult(json: any): Account {
  const result = json?.data?.user?.result;
  if (result) return parseUser(result);
  return parseUser(json);
}
export function parseRelationship(
  json: any,
  id: string,
): { follows_me: boolean; i_follow: boolean } {
  if (!Array.isArray(json))
    throw new Error("Unrecognized relationship response");
  const r = json.find((r: any) => r.id_str === id);
  if (!r || !Array.isArray(r.connections))
    throw new Error("Relationship not returned; cannot establish absence");
  if (
    !r.connections.length ||
    !r.connections.every((v: any) => typeof v === "string")
  )
    throw new Error("Invalid relationship connections");
  const known = new Set([
    "none",
    "following",
    "following_requested",
    "followed_by",
    "blocking",
    "blocked_by",
    "muting",
    "want_retweets",
  ]);
  if (
    r.connections.some((v: string) => !known.has(v)) ||
    (r.connections.includes("none") && r.connections.length !== 1)
  ) {
    throw new Error("Unknown or inconsistent relationship connections");
  }
  return {
    follows_me: r.connections.includes("followed_by"),
    i_follow: r.connections.includes("following"),
  };
}
export function postingEvidence(
  json: any,
  target: string,
  cutoff: number,
  repostsOnly = false,
): { latest: number | null; coverage: number | null; note: string; cursor?: string | null; blocked?: boolean } {
  const times: number[] = [];
  let malformed = false;
  let chronological = false;
  let terminated = false;
  let cursor: string | null = null;
  let entriesSeen = 0;
  const take = (raw: Obj, conversationContext = false) => {
    let t = raw?.tweet ?? raw?.result ?? raw;
    if (t?.__typename === "TweetWithVisibilityResults") t = t.tweet;
    const l = t?.legacy ?? t;
    const actor =
      l?.user_id_str ??
      l?.user?.id_str ??
      t?.core?.user_results?.result?.rest_id;
    if (!actor) {
      malformed = true;
      return;
    }
    if (actor !== target) {
      if (repostsOnly || !conversationContext) malformed = true; // A foreign standalone post may be an undated repost.
      return;
    }
    const at = Date.parse(l?.created_at ?? "");
    if (Number.isFinite(at)) times.push(at);
    else malformed = true;
  };
  if (Array.isArray(json)) {
    chronological = true;
    for (const t of json) take(t);
  } else {
    for (const i of instructions(json)) {
      if (i.type === "TimelinePinEntry") {
        const start = times.length;
        const wasMalformed = malformed;
        const item = i.entry?.content?.itemContent ?? i.entry?.content;
        if (item?.tweet_results?.result) take(item.tweet_results.result);
        // A recent authored pin proves activity. An old pin cannot prove inactivity.
        times.splice(start, times.length - start, ...times.slice(start).filter((t) => t > cutoff));
        malformed = wasMalformed;
        continue;
      }
      if (i.type === "TimelineTerminateTimeline" && i.direction === "Bottom")
        terminated = true;
      if (i.type !== "TimelineAddEntries" && i.type !== "TimelineReplaceEntry")
        continue;
      if (i.type === "TimelineAddEntries") chronological = true;
      for (const e of i.entries ?? [i.entry]) {
        if (!e) continue;
        if (e.entryId?.startsWith("promoted-") || e.content?.itemContent?.promotedMetadata) continue;
        const content = e.content;
        if (content?.cursorType) {
          if (content.cursorType === "Bottom" && typeof content.value === "string") cursor = content.value;
          continue;
        }
        entriesSeen++;
        const items = content?.items
          ? content.items.map((part: Obj) => part.item?.itemContent ?? part.itemContent)
          : [content?.itemContent];
        for (const item of items) {
          if (item?.tweet_results?.result) take(item.tweet_results.result, Array.isArray(content?.items));
          else malformed = true;
        }
      }
    }
  }
  const latest = times.length ? Math.max(...times) : null;
  const metadata = { cursor: cursor === "0" ? null : cursor, blocked: malformed };
  if (latest !== null && latest > cutoff)
    return {
      ...metadata,
      latest,
      coverage: cutoff,
      note: "Recent posting action observed (including replies/reposts).",
    };
  if (latest !== null && !malformed && chronological)
    return {
      ...metadata,
      latest,
      coverage: cutoff,
      note: "Latest visible chronological posting actions predate the cutoff.",
    };
  if (latest === null && !malformed && chronological && (terminated || cursor === "0" || (!Array.isArray(json) && entriesSeen === 0 && cursor === null)))
    return {
      ...metadata,
      latest: null,
      coverage: cutoff,
      note: "X explicitly completed an empty activity timeline.",
    };
  return {
    ...metadata,
    latest,
    coverage: null,
    note: "No adequate posting evidence returned; activity remains unknown.",
  };
}

export const OPERATIONS = [
  "Followers",
  "Following",
  "UserByRestId",
  "UserTweetsAndReplies",
  "UserOriginalsTimeline",
  "UserRepliesTimeline",
  "UserRepostsTimeline",
  "RemoveFollower",
] as const;
export type Operation = (typeof OPERATIONS)[number];
export interface Template {
  source?: "observed" | "bundle";
  id: string;
  variables: Obj;
  features: Obj;
  fieldToggles: Obj;
}
export function discoverOperations(
  script: string,
): Partial<Record<Operation, Template>> {
  const found: Partial<Record<Operation, Template>> = {};
  const definitions = [
    /queryId:\s*["'`]([A-Za-z0-9_-]+)["'`],\s*operationName:\s*["'`]([A-Za-z]+)["'`]/g,
    /params:\s*\{id:\s*["'`]([A-Za-z0-9_-]+)["'`],[\s\S]{0,500}?name:\s*["'`]([A-Za-z]+)["'`],[\s\S]{0,100}?operationKind:/g,
  ];
  for (const m of definitions.flatMap((re) => [...script.matchAll(re)])) {
    if (!OPERATIONS.includes(m[2] as Operation)) continue;
    const nearby = script
      .slice(m.index! + m[0].length, m.index! + 3000)
      .split(/queryId:|params:\{id:/)[0];
    const switches = nearby.match(/featureSwitches:\s*\[([^\]]*)\]/)?.[1] ?? "";
    const features = Object.fromEntries(
      [...switches.matchAll(/["']([^"']+)["']/g)].map((m) => [m[1], false]),
    );
    found[m[2] as Operation] = {
      id: m[1],
      variables: {},
      features,
      fieldToggles: {},
    };
  }
  return found;
}
