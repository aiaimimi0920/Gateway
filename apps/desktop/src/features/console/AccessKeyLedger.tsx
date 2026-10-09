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
import { formatTimestamp } from "./accessWorkspaceFormatting";
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
    <div
      className="nt-key-ledger-scroll"
      tabIndex={0}
      role="region"
      aria-label={t("API Key 列表", "API Key list")}
    >
      <table className="nt-key-ledger">
        <thead>
          <tr>
            <th>{t("名称 / API Key", "Name / API Key")}</th>
            <th>{t("状态", "Status")}</th>
            <th>{t("权益组", "Entitlement groups")}</th>
            <th>{t("剩余额度", "Remaining quota")}</th>
            <th>{t("有效期", "Expires")}</th>
            <th>{t("最近使用", "Last used")}</th>
            <th>{t("操作", "Actions")}</th>
          </tr>
        </thead>
        <tbody>
          {keys.map((key) => {
            const status = accessKeyStatus(key);
            const ids = keyGroupIds(key);
            const groupLabel =
              ids === null
                ? t("未绑定（旧密钥）", "Unbound (legacy)")
                : ids.length === 0
                  ? t("无可用权限", "No access")
                  : ids
                      .map((id) => {
                        const group = catalog.accountGroups?.find(
                          (group) => group.id === id,
                        );
                        return group
                          ? `${group.name || id}${group.enabled ? "" : t("（已停用）", " (disabled)")}`
                          : `${id}${t("（已移除）", " (removed)")}`;
                      })
                      .join(" · ");
            return (
              <tr key={key.id}>
                <td>
                  <strong title={key.displayName}>{key.displayName}</strong>
                  <code>{key.publicKeyPrefix}••••••</code>
                  <small title={`${key.ownerId} · ${key.id}`}>
                    {key.ownerId}
                  </small>
                  {bundlesByKey.has(key.id) ? (
                    <small title={bundlesByKey.get(key.id)!.join(" · ")}>
                      {bundlesByKey.get(key.id)!.join(" · ")}
                    </small>
                  ) : null}
                </td>
                <td>
                  <span className={`nt-key-status nt-key-status--${status}`}>
                    {accessKeyStatusLabel(status, t)}
                  </span>
                </td>
                <td className="nt-key-group-label" title={groupLabel}>
                  {groupLabel}
                </td>
                <td>{accessKeyBalanceLabel(balances.get(key.id), t)}</td>
                <td>
                  {key.expiresAt
                    ? formatTimestamp(key.expiresAt)
                    : t("永不过期", "Never")}
                </td>
                <td>
                  {key.lastUsedAt
                    ? formatTimestamp(key.lastUsedAt)
                    : t("未使用", "Unused")}
                </td>
                <td>
                  <div className="nt-key-row-actions">
                    {catalog.cashQuotaSupported ? (
                      <button
                        className="nt-btn nt-btn--outline"
                        type="button"
                        onClick={() => onBills(key)}
                      >
                        {t("账单", "Bills")}
                      </button>
                    ) : null}
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={
                        locked || status !== "active" || !catalog.accountGroups
                      }
                      onClick={() => onEdit(key)}
                    >
                      {t("编辑", "Edit")}
                    </button>
                    <AccessKeyCopyButton
                      t={t}
                      disabled={locked || status !== "active"}
                      onCopy={() => onCopy(key.id)}
                    />
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={locked || status !== "active"}
                      onClick={() => onAction({ kind: "rotate", key })}
                    >
                      {t("轮换", "Rotate")}
                    </button>
                    <button
                      className="nt-btn nt-btn--outline"
                      type="button"
                      disabled={
                        locked ||
                        status === "revoked" ||
                        !["active", "disabled"].includes(key.status)
                      }
                      onClick={() =>
                        onAction({
                          kind: key.status === "active" ? "disable" : "enable",
                          key,
                        })
                      }
                    >
                      {key.status === "disabled"
                        ? t("启用", "Enable")
                        : t("停用", "Disable")}
                    </button>
                    <button
                      className="nt-btn nt-btn--danger"
                      type="button"
                      disabled={locked}
                      onClick={() => onAction({ kind: "delete", key })}
                    >
                      {t("删除", "Delete")}
                    </button>
                  </div>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
