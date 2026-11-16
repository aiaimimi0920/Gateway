import { useEffect, useState } from "react";
import { Badge, Button, DataTable, EmptyState, Notice, Select, type TableColumn } from "@neuro/ui";
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
  const columns: TableColumn<CashReceipt>[] = [
    {
      key: "details",
      header: t("时间 / 明细", "Time / details"),
      width: "44%",
      rowHeader: true,
      cell: (receipt) => <>
        {formatTimestamp(receipt.createdAt)}
        <CashReceiptDetails receipt={receipt} t={t} />
      </>,
    },
    {
      key: "status",
      header: t("状态", "Status"),
      cell: (receipt) => <Badge tone={receipt.status === "settled" ? "success" : receipt.status === "released" ? "neutral" : "warning"}>
        {status[receipt.status]}
      </Badge>,
    },
    {
      key: "reserved",
      header: t("预占金额", "Reserved amount"),
      cell: (receipt) => formatCashMicros(receipt.reservedMicros),
    },
    {
      key: "settled",
      header: t("结算金额", "Settled amount"),
      cell: (receipt) => receipt.amountMicros === null ? "—" : formatCashMicros(receipt.amountMicros),
    },
  ];
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
          <Notice tone="danger">
            {t("请先登录管理会话。", "Sign in to the management session.")}
          </Notice>
        ) : null}
        {error ? (
          <Notice tone="danger">
            {error}
          </Notice>
        ) : null}
        {token && !rows && !error ? (
          <Notice>{t("读取中…", "Loading…")}</Notice>
        ) : null}
        {token && rows?.length === 0 ? (
          <div role="status"><EmptyState title={t("暂无现金账单记录", "No cash billing records")} /></div>
        ) : null}
        {token && rows && rows.length > 0 ? <>
          <div className="nt-key-bills-toolbar">
            <span>{t(`最近 ${rows.length} 条 · USD`, `Latest ${rows.length} · USD`)}</span>
            <Select compact aria-label={t("账单状态", "Bill status")} value={filter}
              onChange={(event) => { setFilter(event.target.value as typeof filter); setPage(1); }}>
              <option value="all">{t("全部状态", "All statuses")}</option>
              {Object.entries(status).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
            </Select>
          </div>
          <div className="nt-cash-ledger">
            <DataTable
              label={t("最近 100 条账单", "Latest 100 bills")}
              rows={filtered.slice((currentPage - 1) * 10, currentPage * 10)}
              columns={columns}
              getRowKey={(receipt) => receipt.requestId}
              empty={<div role="status"><EmptyState title={t("没有此状态的账单", "No bills with this status")} /></div>}
            />
          </div>
          <ScopePager t={t} page={currentPage} totalPages={totalPages} onPage={setPage}
            navLabel={t("账单分页", "Bills pagination")} />
        </> : null}
        {error ? (
          <Button
            type="button"
            disabled={!token}
            onClick={() => setRetry(retry + 1)}
          >
            {t("重试", "Retry")}
          </Button>
        ) : null}
      </div>
      <footer className="nt-key-bills-footer">
        <span>{t("仅展示最近 100 条记录，不代表全部历史账单。", "Shows up to 100 recent records, not the complete billing history.")}</span>
        <Button onClick={onClose}>{t("完成", "Done")}</Button>
      </footer>
    </CashDialog>
  );
}
