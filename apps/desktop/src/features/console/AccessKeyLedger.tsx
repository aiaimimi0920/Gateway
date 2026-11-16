import { Badge, Button, DataTable, DropdownMenu, IconButton } from "@neuro/ui";
import { Ellipsis } from "lucide-react";
import { useMemo } from "react";
import type {
  ConsoleAccessCatalog,
  ConsoleAccessKey,
} from "../../api/contracts";
import {
  accessKeyBalanceLabel,
  accessKeyStatus,
  accessKeyStatusLabel,
} from "./accessKeyPresentation";
import type { PendingKeyAction } from "./AccessKeyDialogs";
import { AccessKeyCopyButton } from "./AccessKeyCopyButton";
import { formatTimestamp, formatRecentKeyUse } from "./accessWorkspaceFormatting";
import { keyGroupIds } from "./accessKeyGroups";
import type { TranslateFn } from "./accessKeysTypes";

export function AccessKeyLedger({
  keys,
  catalog,
  locked,
  onAction,
  onEdit,
  onBills,
  onCopy,
  t,
}: {
  keys: ConsoleAccessKey[];
  catalog: ConsoleAccessCatalog;
  locked: boolean;
  onAction(action: PendingKeyAction): void;
  onEdit(key: ConsoleAccessKey): void;
  onBills(key: ConsoleAccessKey): void;
  onCopy(id: string): Promise<boolean>;
  t: TranslateFn;
}) {
  const balances = useMemo(
    () =>
      new Map(
        catalog.balances.map((balance) => [balance.accessKeyId, balance]),
      ),
    [catalog.balances],
  );
  const bundlesByKey = useMemo(() => {
    const names = new Map(
      catalog.bundles.map((bundle) => [
        bundle.id,
        bundle.displayName || bundle.slug,
      ]),
    );
    const result = new Map<string, string[]>();
    for (const binding of catalog.keyBundleBindings) {
      const labels = result.get(binding.accessKeyId) ?? [];
      labels.push(names.get(binding.bundleId) ?? binding.bundleId);
      result.set(binding.accessKeyId, labels);
    }
    return result;
  }, [catalog.bundles, catalog.keyBundleBindings]);
  return (
    <div className="nt-key-ledger-scroll">
      <DataTable label={t("API Key 列表", "API Key list")} rows={keys} getRowKey={key => key.id} columns={[
        { key: "name", header: t("密钥", "Key"), cell: key => <div className="nt-key-row-name">
          <strong title={key.displayName}>{key.displayName}</strong>
          <div className="nt-key-row-actions nt-key-identity">
            <code>{key.publicKeyPrefix}••••••</code>
            <AccessKeyCopyButton t={t} disabled={locked || accessKeyStatus(key) !== "active"} onCopy={() => onCopy(key.id)} />
          </div>
          {bundlesByKey.has(key.id) ? <small title={bundlesByKey.get(key.id)!.join(" · ")}>{bundlesByKey.get(key.id)!.join(" · ")}</small> : null}
        </div> },
        { key: "groups", header: t("权益组", "Entitlement groups"), width: "18%", cell: key => {
          const ids = keyGroupIds(key);
          const label = ids === null ? t("未绑定（旧密钥）", "Unbound (legacy)") : ids.length === 0 ? t("无可用权限", "No access")
            : ids.map(id => {
              const group = catalog.accountGroups?.find(group => group.id === id);
              return group ? `${group.name || id}${group.enabled ? "" : t("（已停用）", " (disabled)")}` : `${id}${t("（已移除）", " (removed)")}`;
            }).join(" · ");
          return <span className="nt-key-group-label" title={label}>{label}</span>;
        } },
        { key: "balance", header: t("剩余额度", "Remaining quota"), width: "15%", cell: key => <span className="nt-key-balance">{accessKeyBalanceLabel(balances.get(key.id), t)}</span> },
        { key: "usage", header: t("使用情况", "Usage"), width: "23%", cell: key => <div className="nt-key-usage">
          <span title={key.lastUsedAt ? formatTimestamp(key.lastUsedAt) : undefined}>{t("最近：", "Last: ")}{key.lastUsedAt ? formatRecentKeyUse(key.lastUsedAt) : t("未使用", "Unused")}</span>
          <small className={accessKeyStatus(key) === "expired" ? "nt-key-expired" : undefined}>{t("有效期：", "Expires: ")}{key.expiresAt ? formatTimestamp(key.expiresAt) : t("永久", "Never")}</small>
        </div> },
        { key: "status", header: t("状态", "Status"), width: "84px", cell: key => {
          const status = accessKeyStatus(key);
          return <Badge tone={status === "active" ? "success" : status === "expired" ? "warning" : "neutral"}>{accessKeyStatusLabel(status, t)}</Badge>;
        } },
        { key: "actions", header: t("操作", "Actions"), width: "100px", cell: key => <div className="nt-key-row-actions">
          <Button compact disabled={locked || accessKeyStatus(key) !== "active" || !catalog.accountGroups}
            onClick={() => onEdit(key)}>{t("编辑", "Edit")}</Button>
          <DropdownMenu label={t("密钥操作", "Key actions")}
            trigger={<IconButton compact variant="quiet" disabled={locked} label={t("更多操作", "More actions")}><Ellipsis size={16} /></IconButton>}
            items={[
              ...(catalog.cashQuotaSupported ? [{ id: "bills", label: t("账单", "Bills"), onSelect: () => onBills(key) }] : []),
              { id: "rotate", label: t("轮转", "Rotate"), disabled: locked || accessKeyStatus(key) !== "active", onSelect: () => onAction({ kind: "rotate", key }) },
              { id: "toggle", label: key.status === "disabled" ? t("启用", "Enable") : t("停用", "Disable"), disabled: locked || accessKeyStatus(key) === "revoked" || !["active", "disabled"].includes(key.status), onSelect: () => onAction({ kind: key.status === "active" ? "disable" : "enable", key }) },
              { id: "delete", label: t("删除", "Delete"), danger: true, disabled: locked, onSelect: () => onAction({ kind: "delete", key }) },
            ]} />
        </div> },
      ]} />
    </div>
  );
}
