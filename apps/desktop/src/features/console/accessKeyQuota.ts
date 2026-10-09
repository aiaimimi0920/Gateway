import type {
  ConsoleAccessKeyBalance,
  ConsoleAccessKeyInput,
} from "../../api/contracts";
import type { AccessKeyDraft } from "./accessKeysTypes";
import { formatCashMicros, parseCashMicros } from "./cashAmount";

type QuotaDraft = Pick<AccessKeyDraft, "quotaMode" | "quotaLimit">;

export function keyQuotaDraft(balance?: ConsoleAccessKeyBalance): QuotaDraft {
  const mode = balance?.balanceMode ?? "unlimited";
  if (mode === "unlimited") return { quotaMode: mode, quotaLimit: "" };
  if (mode === "cash_prepaid")
    return {
      quotaMode: mode,
      quotaLimit: balance?.cash
        ? formatCashMicros(balance.cash.totalMicros)
        : "",
    };
  if (mode === "message_prepaid" || mode === "request_prepaid")
    return {
      quotaMode: "message_prepaid",
      quotaLimit: String(
        balance?.totalMessages ?? Math.max(0, balance?.remainingMessages ?? 0),
      ),
    };
  if (mode === "token_prepaid")
    return {
      quotaMode: mode,
      quotaLimit: String(
        balance?.totalTokens ?? Math.max(0, balance?.remainingTokens ?? 0),
      ),
    };
  return { quotaMode: "unchanged", quotaLimit: "" };
}

export function validKeyQuota(draft: QuotaDraft): boolean {
  if (draft.quotaMode === "unlimited" || draft.quotaMode === "unchanged")
    return true;
  if (draft.quotaMode === "cash_prepaid")
    return parseCashMicros(draft.quotaLimit) !== null;
  return (
    /^\d+$/.test(draft.quotaLimit) &&
    Number.isSafeInteger(Number(draft.quotaLimit))
  );
}

export function keyQuotaInput(
  draft: QuotaDraft,
): ConsoleAccessKeyInput["quota"] {
  if (!validKeyQuota(draft))
    throw new Error(
      draft.quotaMode === "cash_prepaid"
        ? "金额必须为非负 USD，最多六位小数"
        : "额度必须为非负整数",
    );
  if (draft.quotaMode === "unchanged") return undefined;
  if (draft.quotaMode === "cash_prepaid")
    return {
      mode: "cash_prepaid",
      limit: parseCashMicros(draft.quotaLimit)!,
      currency: "USD",
    };
  return {
    mode: draft.quotaMode,
    limit: draft.quotaMode === "unlimited" ? null : Number(draft.quotaLimit),
  };
}
