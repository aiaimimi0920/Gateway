import { useEffect, useState } from "react";
import type { ConsoleAccessKey } from "../../api/contracts";
import { CashDialog } from "./CashDialog";
import { CashReceiptDetails } from "./CashReceiptDetails";
import { useCashBillingApi } from "./useCashBillingApi";
import { formatCashMicros } from "./cashAmount";
import { formatTimestamp } from "./accessWorkspaceFormatting";
import type { CashReceipt } from "./cashBillingApi";
import type { TranslateFn } from "./accessKeysTypes";

export function AccessKeyCashDialog({
  accessKey,
  onClose,
  t,
}: {
  accessKey: ConsoleAccessKey;
  onClose(): void;
  t: TranslateFn;
}) {
  const { api, token } = useCashBillingApi();
  const [rows, setRows] = useState<CashReceipt[] | null>(null);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    setRows(null);
    setError("");
    if (token)
      void api
        .ledger(token, accessKey.id, controller.signal)
        .then((result) => {
          if (!controller.signal.aborted) setRows(result);
        })
        .catch(() => {
          if (!controller.signal.aborted)
            setError(
              t("账单读取失败，请重试。", "Could not load bills. Retry."),
            );
        });
    return () => controller.abort();
  }, [api, token, accessKey.id, retry, t]);
  const status = {
    reserved: t("已预占", "Reserved"),
    settled: t("已结算", "Settled"),
    unresolved: t("待核对", "Unresolved"),
    released: t("已释放", "Released"),
  };
  return (
    <CashDialog
      wide
      title={t("Key 账单", "Key bills")}
      description={accessKey.displayName}
      onClose={onClose}
      t={t}
    >
      {!token ? (
        <p role="alert">
          {t("请先登录管理会话。", "Sign in to the management session.")}
        </p>
      ) : null}
      {error ? (
        <div className="nt-alert nt-alert--danger" role="alert">
          {error}
        </div>
      ) : null}
      {token && !rows && !error ? (
        <p role="status">{t("读取中…", "Loading…")}</p>
      ) : null}
      {rows?.length === 0 ? (
        <p role="status">{t("暂无现金账单记录", "No cash billing records")}</p>
      ) : null}
      {rows && rows.length > 0 ? (
        <div
          className="nt-cash-ledger"
          tabIndex={0}
          role="region"
          aria-label={t("最近 100 条账单", "Latest 100 bills")}
        >
          <p>{t("最近 100 条 · USD", "Latest 100 · USD")}</p>
          <table className="nt-key-ledger">
            <thead>
              <tr>
                <th>{t("时间 / 明细", "Time / details")}</th>
                <th>{t("状态", "Status")}</th>
                <th>{t("预占金额", "Reserved amount")}</th>
                <th>{t("结算金额", "Settled amount")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((receipt) => (
                <tr key={receipt.requestId}>
                  <td>
                    {formatTimestamp(receipt.createdAt)}
                    <CashReceiptDetails receipt={receipt} t={t} />
                  </td>
                  <td>{status[receipt.status]}</td>
                  <td>{formatCashMicros(receipt.reservedMicros)}</td>
                  <td>
                    {receipt.amountMicros === null
                      ? "—"
                      : formatCashMicros(receipt.amountMicros)}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      {error ? (
        <button
          type="button"
          className="nt-btn nt-btn--outline"
          disabled={!token}
          onClick={() => setRetry(retry + 1)}
        >
          {t("重试", "Retry")}
        </button>
      ) : null}
    </CashDialog>
  );
}
