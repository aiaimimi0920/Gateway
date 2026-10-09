import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
  ConsoleAccessKeyBalance,
} from "../../api/contracts";
import { cashRemaining, formatCashMicros } from "./cashAmount";
import {
  ACCESS_DEFAULT_KEY_DRAFT,
  type AccessKeyDraft,
  type TranslateFn,
} from "./accessKeysTypes";

export type AccessKeyStatus = "active" | "expired" | "revoked" | "inactive";

export function accessKeyStatus(
  key: ConsoleAccessKey,
  now = Date.now(),
): AccessKeyStatus {
  if (key.revokedAt || key.status === "revoked") return "revoked";
  if (key.expiresAt && Date.parse(key.expiresAt) <= now) return "expired";
  return key.status === "active" ? "active" : "inactive";
}

export function accessKeyStatusLabel(
  status: AccessKeyStatus,
  t: TranslateFn,
): string {
  return {
    active: t("生效", "Active"),
    expired: t("已过期", "Expired"),
    revoked: t("已吊销", "Revoked"),
    inactive: t("已停用", "Disabled"),
  }[status];
}

export function newAccessKeyDraft(
  catalog: ConsoleAccessCatalog | null,
): AccessKeyDraft {
  // These are local attribution namespaces, not invented Platform identities.
  const local = catalog?.storageMode === "local";
  return {
    ...ACCESS_DEFAULT_KEY_DRAFT,
    keyKind: "normal",
    ownerId: local ? "local" : "",
    resolvedProjectId: local ? "local" : "",
    resolvedTenantId: local ? "local" : "",
    bundleIds: [],
    accountGroupIds: [],
  };
}

export function accessKeyDraftValid(draft: AccessKeyDraft): boolean {
  return (
    [
      draft.displayName,
      draft.ownerType,
      draft.ownerId,
      draft.resolvedProjectId,
      draft.resolvedTenantId,
      draft.keyKind,
      draft.publicKeyPrefix,
    ].every((value) => value.trim()) &&
    (!draft.expiresAt || Date.parse(draft.expiresAt) > Date.now()) &&
    draft.accountGroupIds.length > 0
  );
}

export function accessKeyBalanceLabel(
  balance: ConsoleAccessKeyBalance | undefined,
  t: TranslateFn,
): string {
  if (!balance) return "—";
  if (balance.status !== "active") return t("不可用", "Unavailable");
  switch (balance.balanceMode) {
    case "unlimited":
      return t("不限额", "Unlimited");
    case "time_pass":
      return t("按时段", "Time pass");
    case "message_prepaid":
    case "request_prepaid":
      return balance.remainingMessages == null
        ? "—"
        : t(
            `${balance.remainingMessages.toLocaleString()} 次`,
            `${balance.remainingMessages.toLocaleString()} requests`,
          );
    case "token_prepaid":
      return balance.remainingTokens == null
        ? "—"
        : `${balance.remainingTokens.toLocaleString()} tokens`;
    case "cash_prepaid":
      return balance.cash
        ? `${formatCashMicros(cashRemaining(balance.cash))} USD`
        : "—";
    default:
      return "—";
  }
}
