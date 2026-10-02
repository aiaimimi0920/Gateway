import { Snowflake, TriangleAlert, UserPlus, UsersRound } from "lucide-react";
import type { ConsoleCredentialPoolAutomationProvider, ConsoleCredentialRefillDemand } from "../../api/contracts";
import type { AccountsLedgerPilotSection, AccountsLedgerWorkspaceProps } from "./accountsLedgerTypes";
import type { PendingProviderLifecycleAction } from "./ProviderLifecycleActionDialog";
import { ProviderStorageEndpoints } from "./ProviderStorageEndpoints";
import { ProviderLifecycleField } from "./ProviderLifecycleField";

export type ProviderLifecycleBackOptions = {
  section: AccountsLedgerPilotSection;
  active?: boolean;
  discardEdits?: boolean;
  availableCount: number;
  coolingCount: number;
  invalidCount: number;
  automation?: ConsoleCredentialPoolAutomationProvider;
  refill?: ConsoleCredentialRefillDemand;
};
type Props = Pick<AccountsLedgerWorkspaceProps, "editorLocked" | "lifecycleActionsLocked" | "pruneBusyProviderId" |
  "refillBusyProviderId" | "archivePurgeBusyProviderId" | "onUpdateProviderPoolTargetSize" |
  "onUpdateProviderPoolMinSize" | "onUpdateProviderStoragePath" | "onUpdateProviderArchivePath" |
  "onToggleProviderAutoRefill" | "onToggleProviderAutoPrune" | "onToggleProviderPermanentDelete" |
  "onRequestProviderRefill" | "onUpdateProviderStoragePassword" | "t"> & {
  options: ProviderLifecycleBackOptions;
  requestProviderLifecycleAction: (action: PendingProviderLifecycleAction) => void;
};

export function ProviderLifecycleBack({ options, editorLocked, lifecycleActionsLocked, onUpdateProviderPoolTargetSize,
  onUpdateProviderPoolMinSize, onUpdateProviderStoragePath, onUpdateProviderArchivePath,
  onToggleProviderAutoRefill, onToggleProviderAutoPrune, pruneBusyProviderId, refillBusyProviderId,
  archivePurgeBusyProviderId, onToggleProviderPermanentDelete, onRequestProviderRefill,
  onUpdateProviderStoragePassword, requestProviderLifecycleAction, t,
}: Props) {
  const { section, automation, refill } = options;
  const { providerId, providerLabel } = section;
  const archivedCount = refill?.archivedCredentialCount ?? 0;
  const lifecycleAction = (kind: PendingProviderLifecycleAction["kind"]) =>
    requestProviderLifecycleAction({ kind, providerId, providerLabel,
      permanentDeleteEnabled: section.permanentDeleteEnabled, archivedCredentialCount: archivedCount });
  const toggle = (label: string, accessible: string, enabled: boolean, action: () => void) => (
    <div className="nt-provider-lifecycle__toggle">
      <span>{label}</span>
      <button className={enabled ? "nt-switch nt-switch--on" : "nt-switch"} type="button"
        role="switch" aria-checked={enabled} aria-label={`${providerLabel} ${accessible}`}
        disabled={editorLocked} onClick={action}>
        <span className="nt-switch__track" aria-hidden="true"><span className="nt-switch__thumb" /></span>
      </button>
    </div>
  );
  const field = { providerLabel, disabled: editorLocked || options.active === false,
    discardDraft: options.active === false || options.discardEdits === true, t };
  return <div className="nt-provider-lifecycle" aria-label={t(`${providerLabel} 账号生命周期`, `${providerLabel} credential lifecycle`)}>
    <section className="nt-provider-lifecycle__group" aria-label={t("号池容量", "Pool capacity")}>
      <div className="nt-provider-lifecycle__metrics">
        <span className="nt-provider-lifecycle__metric nt-provider-lifecycle__metric--available"><UsersRound size={18} /><span>{t("可用池", "Available")}</span><strong>{options.availableCount}</strong></span>
        <span className="nt-provider-lifecycle__metric nt-provider-lifecycle__metric--cooling"><Snowflake size={18} /><span>{t("冷却池", "Cooling")}</span><strong>{options.coolingCount}</strong></span>
        <span className="nt-provider-lifecycle__metric nt-provider-lifecycle__metric--danger"><TriangleAlert size={18} /><span>{t("失效池", "Invalid")}</span><strong>{options.invalidCount}</strong></span>
      </div>
      <div className="nt-provider-lifecycle__pair nt-provider-lifecycle__inset-rule">
        <ProviderLifecycleField {...field} label={t("最小可用池", "Minimum pool")} numeric value={String(section.poolMinSize ?? 1)}
          maximum={section.poolTargetSize} onSave={onUpdateProviderPoolMinSize ? (value) => onUpdateProviderPoolMinSize(providerId, Number(value)) : undefined} />
        <ProviderLifecycleField {...field} label={t("最大可用池", "Maximum pool")} numeric value={String(section.poolTargetSize)} minimum={Math.max(1, section.poolMinSize ?? 1)}
          onSave={(value) => onUpdateProviderPoolTargetSize(providerId, Number(value))} />
      </div>
    </section>
    <section className="nt-provider-lifecycle__group" aria-label={t("补号与存储", "Refill and storage")}>
      <div className="nt-provider-lifecycle__pair nt-provider-lifecycle__operations">
        <div className="nt-provider-lifecycle__column">
          <ProviderStorageEndpoints providerLabel={providerLabel} label={t("补号通知", "Refill notification")} value={refill?.notificationApi} t={t} />
          <ProviderStorageEndpoints providerLabel={providerLabel} label={t("信息查询", "Pool inquiry")} value={refill?.inquiryApi} t={t} />
        </div>
        <div className="nt-provider-lifecycle__column">
          {toggle(t("自动补号", "Auto refill"), t("自动补号", "auto refill"), section.autoRefillEnabled,
            () => onToggleProviderAutoRefill(providerId, !section.autoRefillEnabled))}
          <div className="nt-provider-lifecycle__command-row">
            <button className="nt-btn nt-btn--secondary nt-provider-lifecycle__command" type="button"
              aria-label={t(`${providerLabel} 手动补号`, `Manually refill ${providerLabel}`)}
              disabled={editorLocked || lifecycleActionsLocked || !refill?.userRequestEnabled || refillBusyProviderId === providerId || options.availableCount >= section.poolTargetSize || Boolean(refill?.outstandingTaskId)}
              onClick={() => onRequestProviderRefill(providerId)}><UserPlus size={17} />
              {refillBusyProviderId === providerId ? t("补号中", "Refilling") : t("补号", "Refill")}
            </button>
          </div>
        </div>
      </div>
      <div className="nt-provider-lifecycle__pair nt-provider-lifecycle__inset-rule">
        <ProviderLifecycleField {...field} label={t("存储路径", "Storage path")} description={t("补号程序提交的凭证 JSON 读取目录", "Directory for credential JSON supplied by refill workers")} value={section.credentialStoragePath ?? refill?.credentialStoragePath ?? ""}
          onSave={onUpdateProviderStoragePath ? (value) => onUpdateProviderStoragePath(providerId, value) : undefined} />
        <ProviderLifecycleField {...field} label={t("存储密码", "Storage password")} description={t("外部补号程序使用的敏感配置；不加密本地文件", "Secret for external refill workers; does not encrypt local files")} secret value="" configured={refill?.storagePasswordConfigured}
          onSave={(value) => onUpdateProviderStoragePassword(providerId, value)} />
      </div>
    </section>
    <section className="nt-provider-lifecycle__group" aria-label={t("删除与归档", "Deletion and archive")}>
      <div className="nt-provider-lifecycle__pair nt-provider-lifecycle__operations">
        <div className="nt-provider-lifecycle__column">
          {toggle(t("自动删除", "Auto delete"), t("自动删除失效号", "auto delete invalid credentials"), section.autoPruneEnabled,
            () => onToggleProviderAutoPrune(providerId, !section.autoPruneEnabled))}
          <div className="nt-provider-lifecycle__command-row">
            <button className="nt-btn nt-btn--danger nt-provider-lifecycle__command" type="button"
              aria-label={t(`${providerLabel} 手动删除失效号`, `Manually delete invalid ${providerLabel} credentials`)}
              disabled={editorLocked || lifecycleActionsLocked || !automation?.driverConfigured || pruneBusyProviderId === providerId}
              onClick={() => lifecycleAction("prune")}>
              {pruneBusyProviderId === providerId ? t("删除中", "Deleting") : t(`删除（${options.invalidCount}）`, `Delete (${options.invalidCount})`)}
            </button>
          </div>
        </div>
        <div className="nt-provider-lifecycle__column">
          {toggle(t("彻底删除", "Permanent delete"), t("彻底删除模式", "permanent deletion mode"), section.permanentDeleteEnabled,
            () => section.permanentDeleteEnabled ? onToggleProviderPermanentDelete(providerId, false) : lifecycleAction("enable-permanent-delete"))}
          <div className="nt-provider-lifecycle__command-row">
            <button className="nt-btn nt-btn--secondary nt-provider-lifecycle__command" type="button"
              aria-label={t(`${providerLabel} 手动清空账号归档`, `Manually purge the ${providerLabel} account archive`)}
              disabled={editorLocked || lifecycleActionsLocked || archivedCount === 0 || archivePurgeBusyProviderId === providerId}
              onClick={() => lifecycleAction("purge-archive")}>
              {archivePurgeBusyProviderId === providerId ? t("清理中", "Purging") : t(`清空（${archivedCount}）`, `Purge (${archivedCount})`)}
            </button>
          </div>
        </div>
      </div>
      <div className="nt-provider-lifecycle__pair nt-provider-lifecycle__inset-rule">
        <ProviderLifecycleField {...field} label={t("归档路径", "Archive path")} description={t("失效凭证删除前写入恢复归档的目录", "Recovery archive written before removing invalid credentials")} value={section.credentialArchivePath ?? refill?.archiveStoragePath ?? ""}
          onSave={onUpdateProviderArchivePath ? (value) => onUpdateProviderArchivePath(providerId, value) : undefined} />
        <ProviderLifecycleField {...field} label={t("归档密码", "Archive password")} secret value=""
          unavailableReason={t("本地归档没有密码加密；云存储认证协议尚未配置", "Local archives are not password encrypted; cloud authentication is not configured")} />
      </div>
    </section>
  </div>;
}
