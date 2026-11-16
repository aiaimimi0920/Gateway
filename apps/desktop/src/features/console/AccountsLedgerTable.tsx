import type { AccountLedgerRow } from "./accountManagementViewModel";
import { Badge, Button, DataTable, type TableColumn } from "@neuro/ui";
import type { TranslateFn } from "./accountsLedgerTypes";
import { providerAccessLabel } from "./providerAccessKind";

export type AccountsLedgerTableProps = {
  t: TranslateFn;
  editorLocked: boolean;
  rows: readonly AccountLedgerRow[];
  onEdit: (providerId: string, accountId: string) => void;
  onAddExplicit: (providerId: string) => void;
};

/** Render legacy account rows without taking ownership of workspace state. */
export function AccountsLedgerTable({
  t,
  editorLocked,
  rows,
  onEdit,
  onAddExplicit,
}: AccountsLedgerTableProps) {
  if (rows.length === 0) {
    return null;
  }

  const columns: TableColumn<AccountLedgerRow>[] = [
    {
      key: "account", header: t("账号", "Account"), width: "32%", cell: row =>
        <div className="nt-ledger-cell nt-ledger-cell--account">
          <strong>{row.displayName}</strong>
          <span>{row.accountId}</span>
          <span>
            {row.hostLabel ?? t("未配置地址", "No base URL")}
            {row.providerPreset ? ` · ${row.providerPreset}` : ""}
          </span>
        </div>
    },
    {
      key: "provider", header: "Provider", width: "26%", cell: row => <div className="nt-ledger-cell">
        <strong>{providerAccessLabel(row)}</strong>
        <span>{row.vendorLabel}</span>
      </div>
    },
    {
      key: "state", header: t("状态", "Status"), width: "18%", cell: row =>
        <Badge tone={row.enabled ? "success" : "warning"}>
          {row.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}
        </Badge>
    },
    {
      key: "actions", header: t("操作", "Actions"), width: "24%", cell: row =>
        <div className="n-row">
          {row.mode === "credential" ? (
            <Button compact wrap
              disabled={editorLocked}
              aria-label={t(`编辑账号 ${row.displayName}`, `Edit account ${row.displayName}`)}
              onClick={() => onEdit(row.providerId, row.accountId)}
            >
              {t("编辑", "Edit")}
            </Button>
          ) : (
            <Button compact wrap
              disabled={editorLocked}
              aria-label={t(
                `为 ${row.providerLabel} 添加显式账号`,
                `Add explicit account for ${row.providerLabel}`,
              )}
              onClick={() => onAddExplicit(row.providerId)}
            >
              {t("添加显式账号", "Add explicit account")}
            </Button>
          )}
        </div>
    },
  ];
  return <DataTable label={t("账号台账表", "Account ledger table")} rows={rows} columns={columns}
    getRowKey={row => `${row.providerId}:${row.accountId}`} />;
}
