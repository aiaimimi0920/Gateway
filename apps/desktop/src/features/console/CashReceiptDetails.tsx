import type { CashReceipt } from "./cashBillingApi";
import { formatCashMicros } from "./cashAmount";
import type { TranslateFn } from "./accessKeysTypes";

export function CashReceiptDetails({
  receipt,
  t,
}: {
  receipt: CashReceipt;
  t: TranslateFn;
}) {
  const usage = receipt.usage;
  const quotes = receipt.settledQuote ? [receipt.settledQuote] : receipt.quotes;
  return (
    <details className="nt-cash-receipt">
      <summary>{t("用量与固定报价", "Usage and frozen quote")}</summary>
      <dl>
        <dt>{t("请求 ID", "Request ID")}</dt>
        <dd>{receipt.requestId}</dd>
        <dt>Key ID</dt>
        <dd>{receipt.accessKeyId}</dd>
        <dt>{t("实际输入 / 输出 Token", "Actual input / output tokens")}</dt>
        <dd>
          {usage ? `${usage.prompt_tokens} / ${usage.completion_tokens}` : "—"}
        </dd>
        <dt>{t("缓存创建 / 读取 Token", "Cache creation / read tokens")}</dt>
        <dd>
          {usage
            ? `${usage.cache_creation_input_tokens ?? "—"} / ${usage.cache_read_input_tokens ?? "—"}`
            : "—"}
        </dd>
        {receipt.reason ? (
          <>
            <dt>{t("原因", "Reason")}</dt>
            <dd>{receipt.reason}</dd>
          </>
        ) : null}
      </dl>
      {quotes.map((quote, index) => (
        <dl key={index}>
          <dt>{t("供应商 / 账户", "Provider / account")}</dt>
          <dd>
            {quote.providerAccountId} / {quote.credentialId}
          </dd>
          <dt>{t("模型", "Model")}</dt>
          <dd>{quote.model}</dd>
          <dt>{t("权益组", "Entitlement group")}</dt>
          <dd>{quote.groupId ?? "—"}</dd>
          <dt>
            {t(
              "输入 / 输出单价（USD / 千 Token）",
              "Input / output price (USD / 1k tokens)",
            )}
          </dt>
          <dd>
            {formatCashMicros(quote.promptMicrosPer1kTokens)} /{" "}
            {formatCashMicros(quote.completionMicrosPer1kTokens)}
          </dd>
          <dt>{t("权益组 / 账户倍率", "Group / account multiplier")}</dt>
          <dd>
            {formatCashMicros(quote.groupMultiplierPpm)} /{" "}
            {formatCashMicros(quote.accountMultiplierPpm)}
          </dd>
          <dt>{t("价格来源", "Price source")}</dt>
          <dd>{quote.priceSource}</dd>
        </dl>
      ))}
    </details>
  );
}
