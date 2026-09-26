import type { ConsoleAccountGroupSummary } from "../../api/contracts";
import type { UiLocale } from "../../i18n/UiLocaleProvider";

/** `active | degraded | cooling | blocked` mapped to the pool-bar vocabulary. */
export function credentialStatusLabel(status: string | null, locale: UiLocale): string | null {
  switch (status?.trim().toLowerCase()) {
    case "active":
      return locale === "zh-CN" ? "正常" : "active";
    // Degraded carries a short cooldown, so it belongs in the rate-limited band
    // of the pool bar rather than the invalid band.
    case "degraded":
      return locale === "zh-CN" ? "降级冷却" : "degraded cooldown";
    case "cooling":
      return locale === "zh-CN" ? "限流等待恢复" : "cooling down";
    case "blocked":
      return locale === "zh-CN" ? "失效" : "blocked";
    default:
      return null;
  }
}

/**
 * Highest billing multiplier among the enabled groups a credential belongs to.
 * Several groups can bill the same credential at different rates, so the card
 * shows the ceiling rather than inventing a blended rate.
 */
export function resolveBillingMultiplier(
  summary: ConsoleAccountGroupSummary | null,
  credentialRefs: Iterable<string>,
): number {
  const refs = new Set(credentialRefs);
  if (refs.size === 0 || !summary) {
    return 1;
  }
  let multiplier: number | null = null;
  for (const group of summary.accountGroups) {
    if (!group.enabled) {
      continue;
    }
    if (!group.providerCredentialIds.some((credentialId) => refs.has(credentialId))) {
      continue;
    }
    const candidate = group.billingMultiplier;
    if (typeof candidate !== "number" || !Number.isFinite(candidate) || candidate < 0) {
      continue;
    }
    multiplier = multiplier === null ? candidate : Math.max(multiplier, candidate);
  }
  return multiplier ?? 1;
}

/** `刚刚 / 12 分钟前 / 3 小时前 / 5 天前`, or the raw date beyond a month. */
export function formatRelativeSince(
  timestamp: string | null,
  locale: UiLocale,
  now = Date.now(),
): string | null {
  if (!timestamp) {
    return null;
  }
  const parsed = Date.parse(timestamp);
  if (!Number.isFinite(parsed)) {
    return null;
  }
  const elapsedSeconds = Math.floor((now - parsed) / 1000);
  if (elapsedSeconds < 0) {
    return locale === "zh-CN" ? "刚刚" : "just now";
  }
  if (elapsedSeconds < 60) {
    return locale === "zh-CN" ? "刚刚" : "just now";
  }
  const minutes = Math.floor(elapsedSeconds / 60);
  if (minutes < 60) {
    return locale === "zh-CN" ? `${minutes} 分钟前` : `${minutes}m ago`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return locale === "zh-CN" ? `${hours} 小时前` : `${hours}h ago`;
  }
  const days = Math.floor(hours / 24);
  if (days <= 30) {
    return locale === "zh-CN" ? `${days} 天前` : `${days}d ago`;
  }
  return new Date(parsed).toLocaleDateString(locale);
}
