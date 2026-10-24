import { useEffect, useState } from "react";
import type { ConsoleAccessKey } from "../../api/contracts";
import { CashDialog } from "./CashDialog";
import { CashReceiptDetails } from "./CashReceiptDetails";
import { useCashBillingApi } from "./useCashBillingApi";
import { formatCashMicros } from "./cashAmount";
import { formatTimestamp } from "./accessWorkspaceFormatting";
import type { CashReceipt } from "./cashBillingApi";
import type { TranslateFn } from "./accessKeysTypes";
import { ScopePager } from "./EntitlementScopePager";
import "./accessKeyCashDialog.css";

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
  const [filter, setFilter] = useState<"all" | CashReceipt["status"]>("all");
  const [page, setPage] = useState(1);
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
  const filtered = (rows ?? []).filter((row) => filter === "all" || row.status === filter);
  const totalPages = Math.max(1, Math.ceil(filtered.length / 10));
  const currentPage = Math.min(page, totalPages);
  return (
    <CashDialog
      wide
      className="nt-cash-dialog--key-bills"
      title={t("Key 账单", "Key bills")}
      description={accessKey.displayName}
      onClose={onClose}
      t={t}
    >
      <div className="nt-key-bills-body">
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
      {token && rows?.length === 0 ? (
        <p role="status">{t("暂无现金账单记录", "No cash billing records")}</p>
      ) : null}
      {token && rows && rows.length > 0 ? <>
        <div className="nt-key-bills-toolbar">
          <span>{t(`最近 ${rows.length} 条 · USD`, `Latest ${rows.length} · USD`)}</span>
          <select className="nt-input" aria-label={t("账单状态", "Bill status")} value={filter}
            onChange={(event) => { setFilter(event.target.value as typeof filter); setPage(1); }}>
            <option value="all">{t("全部状态", "All statuses")}</option>
            {Object.entries(status).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
          </select>
        </div>
        <div
          className="nt-cash-ledger"
          tabIndex={0}
          role="region"
          aria-label={t("最近 100 条账单", "Latest 100 bills")}
        >
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
              {filtered.slice((currentPage - 1) * 10, currentPage * 10).map((receipt) => (
                <tr key={receipt.requestId}>
                  <td>
                    {formatTimestamp(receipt.createdAt)}
                    <CashReceiptDetails receipt={receipt} t={t} />
                  </td>
                  <td><span className={`nt-key-bill-status nt-key-bill-status--${receipt.status}`}>{status[receipt.status]}</span></td>
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
          {filtered.length === 0 && <p role="status">{t("没有此状态的账单", "No bills with this status")}</p>}
        </div>
        <ScopePager t={t} page={currentPage} totalPages={totalPages} onPage={setPage}
          navLabel={t("账单分页", "Bills pagination")} />
      </> : null}
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
      </div>
      <footer className="nt-key-bills-footer">
        <span>{t("仅展示最近 100 条记录，不代表全部历史账单。", "Shows up to 100 recent records, not the complete billing history.")}</span>
        <button type="button" className="nt-btn nt-btn--outline" onClick={onClose}>{t("完成", "Done")}</button>
      </footer>
    </CashDialog>
  );
}
