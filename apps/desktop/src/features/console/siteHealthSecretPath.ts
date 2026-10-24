import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";

// A profile's array index is not a secret identity. Legacy and explicit profiles
// intentionally have separate paths, even when the explicit profile binds all groups.
export function remapSiteHealthSecretPath(
  active: ConsoleRouteDocument,
  draft: ConsoleRouteDocument,
  path: string,
): string | null {
  const match = /^\/providers\/(\d+)\/site_health\/(auth\/token|profiles\/(\d+)\/policy\/auth\/token)$/.exec(path);
  if (!match) return null;
  const source = active.providers[Number(match[1])];
  if (!isRecord(source)) return null;
  const targetIndex = draft.providers.findIndex((p) => isRecord(p) && p.id === source.id);
  const target = draft.providers[targetIndex];
  if (!isRecord(target) || !isRecord(target.site_health)) return null;
  if (match[3] === undefined) return `/providers/${targetIndex}/site_health/auth/token`;
  if (!isRecord(source.site_health) || !Array.isArray(source.site_health.profiles)) return null;
  const profile = source.site_health.profiles[Number(match[3])];
  if (!isRecord(profile) || typeof profile.id !== "string" || !Array.isArray(target.site_health.profiles)) return null;
  const index = target.site_health.profiles.findIndex((p) => isRecord(p) && p.id === profile.id);
  return index < 0 ? null : `/providers/${targetIndex}/site_health/profiles/${index}/policy/auth/token`;
}
