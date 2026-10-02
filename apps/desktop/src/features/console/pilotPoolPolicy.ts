import { normalizeBucketKey, optionalString } from "./routeAccountCatalog";
import { isRecord } from "./routeDocument";

export type PilotIdentityCategoryDefinition = {
  id: string;
  label: string;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
};

export type PilotProviderPolicyDefinition = {
  poolMinSize: number;
  poolTargetSize: number;
  autoRefillEnabled: boolean;
  autoPruneEnabled: boolean;
  permanentDeleteEnabled: boolean;
};

export const DEFAULT_PILOT_POOL_TARGET_SIZE = 30;
export const DEFAULT_PROVIDER_POOL_MAX_SIZE = 100;
export const DEFAULT_PROVIDER_POOL_MIN_SIZE = 1;

const CODEX_IDENTITY_CATEGORY_DEFAULTS: PilotIdentityCategoryDefinition[] = [
  {
    id: "free",
    label: "Free",
    poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
    autoRefillEnabled: false,
    autoPruneEnabled: false,
  },
  {
    id: "plus",
    label: "Plus",
    poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
    autoRefillEnabled: false,
    autoPruneEnabled: false,
  },
];

export function normalizePilotPoolTargetSize(
  value: unknown,
  fallback = DEFAULT_PILOT_POOL_TARGET_SIZE,
): number {
  if (typeof value === "number" && Number.isSafeInteger(value) && value >= 1) {
    return Math.floor(value);
  }
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value);
    if (Number.isSafeInteger(parsed) && parsed >= 1) {
      return Math.floor(parsed);
    }
  }
  return fallback;
}

export function readPilotProviderPolicy(
  provider: Record<string, unknown>,
): PilotProviderPolicyDefinition {
  return {
    poolTargetSize: normalizePilotPoolTargetSize(provider.pool_target_size, DEFAULT_PROVIDER_POOL_MAX_SIZE),
    poolMinSize: typeof provider.pool_min_size === "number" && Number.isSafeInteger(provider.pool_min_size) && provider.pool_min_size >= 0
      ? provider.pool_min_size : DEFAULT_PROVIDER_POOL_MIN_SIZE,
    autoRefillEnabled: optionalBoolean(provider, "auto_refill_enabled") ?? false,
    autoPruneEnabled: optionalBoolean(provider, "auto_prune_enabled") ?? false,
    permanentDeleteEnabled:
      optionalBoolean(provider, "credential_permanent_delete_enabled") ?? false,
  };
}

export function normalizePilotCategoryId(value: string, fallback: string): string {
  const normalized = normalizeBucketKey(value);
  return normalized.length > 0 ? normalized : fallback;
}

export function isCodexPilotProvider(providerId: string, providerLabel: string): boolean {
  const normalizedId = providerId.trim().toLowerCase();
  const normalizedLabel = providerLabel.trim().toLowerCase();
  return normalizedId === "codex" || normalizedLabel === "codex";
}

export function readPilotIdentityCategories(
  provider: Record<string, unknown>,
  providerId: string,
  providerLabel: string,
): PilotIdentityCategoryDefinition[] {
  const rawCategories = provider.credential_identity_categories;
  const definitions = Array.isArray(rawCategories)
    ? rawCategories
        .map((entry, index) => {
          if (typeof entry === "string" && entry.trim().length > 0) {
            const label = entry.trim();
            return {
              id: normalizePilotCategoryId(label, `identity-${index + 1}`),
              label,
              poolTargetSize: DEFAULT_PILOT_POOL_TARGET_SIZE,
              autoRefillEnabled: false,
              autoPruneEnabled: false,
            } satisfies PilotIdentityCategoryDefinition;
          }
          if (!isRecord(entry)) {
            return null;
          }
          const label = optionalString(entry, "label") ?? optionalString(entry, "name");
          if (!label) {
            return null;
          }
          return {
            id: optionalString(entry, "id") ?? normalizePilotCategoryId(label, `identity-${index + 1}`),
            label,
            poolTargetSize: normalizePilotPoolTargetSize(entry.pool_target_size),
            autoRefillEnabled: optionalBoolean(entry, "auto_refill_enabled") ?? false,
            autoPruneEnabled: optionalBoolean(entry, "auto_prune_enabled") ?? false,
          } satisfies PilotIdentityCategoryDefinition;
        })
        .filter(
          (entry): entry is PilotIdentityCategoryDefinition =>
            entry !== null && entry.id.trim().length > 0,
        )
    : [];

  const seenIds = new Set<string>();
  const deduped = definitions.filter((entry) => {
    if (seenIds.has(entry.id)) return false;
    seenIds.add(entry.id);
    return true;
  });
  if (deduped.length > 0) {
    return deduped;
  }
  return isCodexPilotProvider(providerId, providerLabel)
    ? CODEX_IDENTITY_CATEGORY_DEFAULTS
    : [];
}

export function optionalBoolean(record: Record<string, unknown>, key: string): boolean | null {
  const value = record[key];
  return typeof value === "boolean" ? value : null;
}
