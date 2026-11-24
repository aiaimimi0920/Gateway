import { isRecord } from "./routeDocument";

export const ROUTE_EDIT_CONFLICT = "配置已在其他位置更新，且与你修改的内容冲突。你的修改仍保留，请核对最新配置后重试。";

export function sameRouteValue(left: unknown, right: unknown): boolean {
  if (left === right) return true;
  if (Array.isArray(left) && Array.isArray(right)) return left.length === right.length && left.every((v, i) => sameRouteValue(v, right[i]));
  if (!isRecord(left) || !isRecord(right)) return false;
  const keys = Object.keys(left);
  return keys.length === Object.keys(right).length && keys.every(key => Object.hasOwn(right, key) && sameRouteValue(left[key], right[key]));
}

function conflict(): never { throw new Error(ROUTE_EDIT_CONFLICT); }

function identityKey(path: string): string | null {
  if (path === "/providers" || path === "/account_groups" || /^\/providers\/[^/]+\/credentials$/.test(path)) return "id";
  return null;
}

function identified(items: unknown[], key: string): Map<string, unknown> {
  const result = new Map<string, unknown>();
  for (const item of items) {
    if (!isRecord(item) || typeof item[key] !== "string" || !item[key] || result.has(item[key])) conflict();
    result.set(item[key], item);
  }
  return result;
}

/** Three-way merge: only user changes are replayed; overlapping edits stay visible. */
export function mergeRouteValue(base: unknown, draft: unknown, latest: unknown, path = ""): unknown {
  if (sameRouteValue(draft, base)) return latest;
  if (sameRouteValue(latest, base) || sameRouteValue(draft, latest)) return draft;
  if (isRecord(base) && isRecord(draft) && isRecord(latest)) {
    const entries: [string, unknown][] = [];
    for (const key of new Set([...Object.keys(latest), ...Object.keys(draft), ...Object.keys(base)])) {
      const own = (value: Record<string, unknown>) => Object.hasOwn(value, key) ? value[key] : undefined;
      const value = mergeRouteValue(own(base), own(draft), own(latest), `${path}/${key.replaceAll("~", "~0").replaceAll("/", "~1")}`);
      if (value !== undefined) entries.push([key, value]);
    }
    // Model/alias keys are data, including __proto__ and constructor.
    return Object.fromEntries(entries);
  }
  const key = identityKey(path);
  if (key && Array.isArray(base) && Array.isArray(draft) && Array.isArray(latest)) {
    const original = identified(base, key), local = identified(draft, key), remote = identified(latest, key);
    const shared = [...original.keys()].filter(id => local.has(id) && remote.has(id));
    const sharedSet = new Set(shared);
    const order = (map: Map<string, unknown>) => [...map.keys()].filter(id => sharedSet.has(id));
    const localOrder = order(local), remoteOrder = order(remote);
    const localMoved = !sameRouteValue(localOrder, shared), remoteMoved = !sameRouteValue(remoteOrder, shared);
    if (localMoved && remoteMoved && !sameRouteValue(localOrder, remoteOrder)) conflict();
    const ids = localMoved ? [...local.keys(), ...remote.keys()] : [...remote.keys(), ...local.keys()];
    const result: unknown[] = [];
    for (const id of new Set(ids)) {
      const value = mergeRouteValue(original.get(id), local.get(id), remote.get(id), `${path}/${encodeURIComponent(id)}`);
      if (value !== undefined) result.push(value);
    }
    return result;
  }
  // Ordered routes, model priority lists and opaque arrays are indivisible edits.
  return conflict();
}
