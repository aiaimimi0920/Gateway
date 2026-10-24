import type {
  ConsoleRouteConfigResponse,
  ConsoleRouteDocument,
  ConsoleSecretPatch,
} from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { siteHealthConfigSchema, type ProbePolicy } from "./siteHealth";

export type SiteHealthSecretChange = { profileId: string | null } & (
  | { operation: "replace"; value: string }
  | { operation: "clear" }
  | { operation: "keep" }
);
export type SiteHealthSecretEdit = Exclude<
  SiteHealthSecretChange,
  { operation: "keep" }
> & { providerId: string; audience: string };
export const healthSecretIdentity = (
  providerId: string,
  profileId: string | null,
) => JSON.stringify([providerId, profileId]);

// Array slots are resolved only at commit time; a pending secret cannot follow a
// deleted/reordered plan or a destination edited later in the raw JSON editor.
export function healthSecretTarget(
  document: ConsoleRouteDocument,
  providerId: string,
  profileId: string | null,
) {
  const index = document.providers.findIndex(
    (p) => isRecord(p) && p.id === providerId,
  );
  const provider = document.providers[index];
  if (!isRecord(provider)) return null;
  const parsed = siteHealthConfigSchema.safeParse(provider.site_health);
  if (!parsed.success) return null;
  const profileIndex = parsed.data.profiles.findIndex(
    (p) => p.id === profileId,
  );
  const policy =
    profileId === null
      ? parsed.data
      : parsed.data.profiles[profileIndex]?.policy;
  if (!policy) return null;
  const suffix =
    profileId === null
      ? "auth/token"
      : `profiles/${profileIndex}/policy/auth/token`;
  return {
    path: `/providers/${index}/site_health/${suffix}`,
    audience: healthSecretAudience(provider, policy),
  };
}

export function healthSecretAudience(
  provider: Record<string, unknown>,
  policy: ProbePolicy,
): string {
  const raw = policy.site_url ?? String(provider.base_url ?? "");
  let origin = raw || `preset:${String(provider.preset ?? "")}`;
  try {
    origin = new URL(raw).origin;
  } catch {
    /* Unexpanded environment references stay exact. */
  }
  return JSON.stringify([
    origin,
    policy.adapter,
    policy.auth.kind,
    policy.auth.kind === "api_key"
      ? (policy.auth.header ?? "x-api-key").toLowerCase()
      : null,
    policy.allow_private_network,
  ]);
}

export function buildSiteHealthSecretPatches(
  document: ConsoleRouteDocument,
  patches: ConsoleSecretPatch[],
  edits: SiteHealthSecretEdit[],
): ConsoleSecretPatch[] {
  const result = new Map(patches.map((patch) => [patch.path, patch]));
  for (const edit of edits) {
    const target = healthSecretTarget(
      document,
      edit.providerId,
      edit.profileId,
    );
    if (!target || target.audience !== edit.audience) continue;
    result.set(
      target.path,
      edit.operation === "clear"
        ? { path: target.path, operation: "clear" }
        : { path: target.path, operation: "replace", value: edit.value },
    );
  }
  return [...result.values()];
}

export function siteHealthSecretEditError(
  document: ConsoleRouteDocument,
  edits: SiteHealthSecretEdit[],
): string | null {
  for (const edit of edits) {
    const target = healthSecretTarget(
      document,
      edit.providerId,
      edit.profileId,
    );
    if (target && target.audience !== edit.audience)
      return "监控目标或认证已改变，请重新替换或清空监控密钥。";
  }
  return null;
}

export function configuredHealthSecrets(
  config: ConsoleRouteConfigResponse | null,
): ReadonlySet<string> {
  const result = new Set<string>();
  for (const secret of config?.routeConfig.secrets ?? []) {
    const match =
      /^\/providers\/(\d+)\/site_health\/(auth\/token|profiles\/(\d+)\/policy\/auth\/token)$/.exec(
        secret.path,
      );
    if (!match || !secret.configured) continue;
    const provider = config?.routeConfig.document.providers[Number(match[1])];
    if (!isRecord(provider) || typeof provider.id !== "string") continue;
    if (match[3] === undefined)
      result.add(healthSecretIdentity(provider.id, null));
    else {
      const health = siteHealthConfigSchema.safeParse(provider.site_health);
      const profile = health.success
        ? health.data.profiles[Number(match[3])]
        : undefined;
      if (profile) result.add(healthSecretIdentity(provider.id, profile.id));
    }
  }
  return result;
}

export function validateHealthSecretChange(change: SiteHealthSecretChange) {
  if (
    change.operation === "replace" &&
    (!change.value.trim() ||
      change.value.length > 8192 ||
      /[\u0000-\u001f\u007f]/.test(change.value))
  ) {
    throw new Error("监控密钥不能为空、包含控制字符或超过 8192 字符。");
  }
}
