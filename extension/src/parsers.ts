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
    legacy = bool(l.verified);
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
    bio: text(l.description ?? u.profile_bio?.description),
    followers: count(l.followers_count),
    following_count: count(l.friends_count),
    posts: count(l.statuses_count),
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
    terminated = false;
  for (const i of inst) {
    if (i.type === "TimelineTerminateTimeline" && i.direction === "Bottom") {
      terminated = true;
      recognized = true;
    }
    if (i.type === "TimelineAddEntries" || i.type === "TimelineReplaceEntry") {
      recognized = true;
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
  const complete = terminated || cursor === null || cursor === "0";
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
): { latest: number | null; coverage: number | null; note: string } {
  const times: number[] = [];
  let malformed = false;
  let chronological = false;
  const take = (raw: Obj) => {
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
    if (actor !== target) return;
    const at = Date.parse(l?.created_at ?? "");
    if (Number.isFinite(at)) times.push(at);
    else malformed = true;
  };
  if (Array.isArray(json)) {
    chronological = true;
    for (const t of json) take(t);
  } else {
    for (const i of instructions(json)) {
      if (i.type !== "TimelineAddEntries" && i.type !== "TimelineReplaceEntry")
        continue;
      if (i.type === "TimelineAddEntries") chronological = true;
      for (const e of i.entries ?? [i.entry])
        if (e) {
          const content = e.content;
          if (content?.cursorType) continue;
          const item = content?.itemContent;
          if (item?.tweet_results?.result) take(item.tweet_results.result);
          else {
            // Modules, tombstones and unknown entry shapes cannot prove negative activity.
            malformed = true;
            walk(content, (o) => {
              if (o.tweet_results?.result) take(o.tweet_results.result);
            });
          }
        }
    }
  }
  const latest = times.length ? Math.max(...times) : null;
  if (latest !== null && latest > cutoff)
    return {
      latest,
      coverage: cutoff,
      note: "Recent posting action observed (including replies/reposts).",
    };
  if (latest !== null && !malformed && chronological)
    return {
      latest,
      coverage: cutoff,
      note: "Latest visible chronological posting actions predate the cutoff.",
    };
  return {
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
  "RemoveFollower",
] as const;
export type Operation = (typeof OPERATIONS)[number];
export interface Template {
  id: string;
  variables: Obj;
  features: Obj;
  fieldToggles: Obj;
}
export function discoverOperations(
  script: string,
): Partial<Record<Operation, Template>> {
  const found: Partial<Record<Operation, Template>> = {};
  const re =
    /queryId:\s*["']([A-Za-z0-9_-]+)["'],\s*operationName:\s*["']([A-Za-z]+)["']/g;
  for (const m of script.matchAll(re)) {
    if (!OPERATIONS.includes(m[2] as Operation)) continue;
    const nearby =
      script.slice(m.index!, m.index! + 3000).split("queryId:")[1] ?? "";
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
