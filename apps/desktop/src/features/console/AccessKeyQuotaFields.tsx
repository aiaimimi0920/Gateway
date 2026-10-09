import type { ConsoleAccessKeyBalance } from "../../api/contracts";
import { useId } from "react";
import type { AccessKeyDraft, TranslateFn } from "./accessKeysTypes";
import { cashRemaining, formatCashMicros } from "./cashAmount";

export function AccessKeyQuotaFields({
  draft,
  onChange,
  balance,
  cashQuotaSupported = false,
  t,
}: {
  draft: AccessKeyDraft;
  onChange(value: Partial<AccessKeyDraft>): void;
  balance?: ConsoleAccessKeyBalance;
  cashQuotaSupported?: boolean;
  t: TranslateFn;
}) {
  const id = useId();
  const limited =
    draft.quotaMode === "message_prepaid" ||
    draft.quotaMode === "token_prepaid" ||
    draft.quotaMode === "cash_prepaid";
  const token = draft.quotaMode === "token_prepaid";
  const cash = draft.quotaMode === "cash_prepaid";
  const total = token ? balance?.totalTokens : balance?.totalMessages;
  const remaining = token
    ? balance?.remainingTokens
    : balance?.remainingMessages;
  return (
    <div className="nt-key-quota-fields">
      <label className="nt-field">
        <span>{t("额度限制", "Quota")}</span>
        <select
          className="nt-input"
          value={draft.quotaMode}
          onChange={(event) =>
            onChange({
              quotaMode: event.target.value as AccessKeyDraft["quotaMode"],
              // Switching units never silently turns a token count into dollars.
              quotaLimit:
                event.target.value === "cash_prepaid"
                  ? balance?.cash
                    ? formatCashMicros(balance.cash.totalMicros)
                    : ""
                  : event.target.value === "token_prepaid"
                    ? balance?.totalTokens == null
                      ? ""
                      : String(balance.totalTokens)
                    : event.target.value === "message_prepaid"
                      ? balance?.totalMessages == null
                        ? ""
                        : String(balance.totalMessages)
                      : "",
            })
          }
        >
          {draft.quotaMode === "unchanged" ? (
            <option value="unchanged">
              {t("保持现有额度", "Keep current quota")}
            </option>
          ) : null}
          <option value="unlimited">{t("不限额", "Unlimited")}</option>
          <option value="message_prepaid">
            {t("请求次数", "Request count")}
          </option>
          <option value="token_prepaid">
            {t("Token 总量", "Total tokens")}
          </option>
          {cashQuotaSupported || cash ? (
            <option value="cash_prepaid" disabled={!cashQuotaSupported}>
              {t("总花费", "Total spend")}
            </option>
          ) : null}
        </select>
      </label>
      {limited ? (
        <label className="nt-field">
          <span id={`${id}-label`}>
            {cash
              ? t("总花费（USD）", "Total spend (USD)")
              : token
                ? t("总 Token 额度", "Total token allowance")
                : t("总请求次数", "Total request allowance")}
          </span>
          <input
            className="nt-input"
            aria-labelledby={`${id}-label`}
            aria-describedby={
              cash
                ? balance?.cash
                  ? `${id}-usage`
                  : undefined
                : total != null && remaining != null
                  ? `${id}-usage`
                  : undefined
            }
            type="number"
            min="0"
            max={
              cash
                ? Number.MAX_SAFE_INTEGER / 1_000_000
                : Number.MAX_SAFE_INTEGER
            }
            step={cash ? "0.000001" : "1"}
            required
            value={draft.quotaLimit}
            onChange={(event) => onChange({ quotaLimit: event.target.value })}
          />
          {cash && balance?.cash ? (
            <small id={`${id}-usage`}>
              {t(
                `已结算 ${formatCashMicros(balance.cash.spentMicros)} · 预占 ${formatCashMicros(balance.cash.reservedMicros)} · 可用 ${formatCashMicros(cashRemaining(balance.cash))} USD`,
                `Settled ${formatCashMicros(balance.cash.spentMicros)} · Reserved ${formatCashMicros(balance.cash.reservedMicros)} · Available ${formatCashMicros(cashRemaining(balance.cash))} USD`,
              )}
            </small>
          ) : !cash && total != null && remaining != null ? (
            <small id={`${id}-usage`}>
              {t(
                `已用 ${Math.max(0, total - remaining).toLocaleString()} · 剩余 ${remaining.toLocaleString()}`,
                `Used ${Math.max(0, total - remaining).toLocaleString()} · Remaining ${remaining.toLocaleString()}`,
              )}
            </small>
          ) : null}
        </label>
      ) : null}
    </div>
  );
}
