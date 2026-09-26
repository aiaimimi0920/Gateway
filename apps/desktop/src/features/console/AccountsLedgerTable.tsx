import type { AccountLedgerRow } from "./accountManagementViewModel";
import type { TranslateFn } from "./accountsLedgerTypes";

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

  return (
    <section
      className="nt-ledger-table"
      role="table"
      aria-label={t("账号台账表", "Account ledger table")}
    >
      <div className="nt-ledger-table__head" role="row">
        <span role="columnheader">{t("账号", "Account")}</span>
        <span role="columnheader">Provider</span>
        <span role="columnheader">{t("状态", "Status")}</span>
        <span role="columnheader">{t("操作", "Actions")}</span>
      </div>

      {rows.map((row) => (
        <div
          className="nt-ledger-table__row"
          role="row"
          key={`${row.providerId}:${row.accountId}`}
        >
          <div className="nt-ledger-cell nt-ledger-cell--account" role="cell">
            <strong>{row.displayName}</strong>
            <span>{row.accountId}</span>
            <span>
              {row.hostLabel ?? t("未配置地址", "No base URL")}
              {row.providerPreset ? ` · ${row.providerPreset}` : ""}
            </span>
          </div>
          <div className="nt-ledger-cell" role="cell">
            <strong>{row.providerLabel}</strong>
            <span>{row.vendorLabel}</span>
          </div>
          <div className="nt-ledger-cell" role="cell">
            <span
              className={
                row.enabled
                  ? "nt-badge nt-badge--success"
                  : "nt-badge nt-badge--warning"
              }
            >
              {row.enabled ? t("已启用", "Enabled") : t("已停用", "Disabled")}
            </span>
          </div>
          <div className="nt-ledger-cell nt-ledger-cell--actions" role="cell">
            <div className="nt-actions">
              {row.mode === "credential" ? (
                <button
                  className="nt-btn nt-btn--outline"
                  type="button"
                  disabled={editorLocked}
                  aria-label={t(`编辑账号 ${row.displayName}`, `Edit account ${row.displayName}`)}
                  onClick={() => onEdit(row.providerId, row.accountId)}
                >
                  {t("编辑", "Edit")}
                </button>
              ) : (
                <button
                  className="nt-btn nt-btn--secondary"
                  type="button"
                  disabled={editorLocked}
                  aria-label={t(
                    `为 ${row.providerLabel} 添加显式账号`,
                    `Add explicit account for ${row.providerLabel}`,
                  )}
                  onClick={() => onAddExplicit(row.providerId)}
                >
                  {t("添加显式账号", "Add explicit account")}
                </button>
              )}
            </div>
          </div>
        </div>
      ))}
    </section>
  );
}
