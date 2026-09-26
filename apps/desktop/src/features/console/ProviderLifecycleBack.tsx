import { Archive, ArchiveX, Copy, ShieldX, Snowflake, UserMinus, UserPlus, UsersRound } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";
import type { ConsoleCredentialPoolAutomationProvider, ConsoleCredentialRefillDemand } from "../../api/contracts";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";
import type { PendingProviderLifecycleAction } from "./ProviderLifecycleActionDialog";
import { pushAppToast } from "../../components/AppToast";
import { ProviderStorageEndpoints } from "./ProviderStorageEndpoints";

export type ProviderLifecycleBackOptions = {
    section: AccountsLedgerPilotSection;
    availableCount: number;
    coolingCount: number;
    invalidCount: number;
    automation?: ConsoleCredentialPoolAutomationProvider;
    refill?: ConsoleCredentialRefillDemand;
  };
type StoragePasswordDraft = { editing: boolean; value: string };
type ProviderLifecycleBackProps = {
  options: ProviderLifecycleBackOptions;
  poolTargetDrafts: Readonly<Record<string, string>>;
  setPoolTargetDrafts: Dispatch<SetStateAction<Record<string, string>>>;
  commitPoolTargetDraft: (policyKey: string, committedValue: number, onCommit: (nextTargetSize: number) => void) => void;
  storagePasswordDrafts: Readonly<Record<string, StoragePasswordDraft>>;
  setStoragePasswordDrafts: Dispatch<SetStateAction<Record<string, StoragePasswordDraft>>>;
  editorLocked: boolean;
  pruneBusyProviderId: string | null;
  refillBusyProviderId: string | null;
  archivePurgeBusyProviderId: string | null;
  onUpdateProviderPoolTargetSize: (providerId: string, nextTargetSize: number) => void;
  onToggleProviderAutoRefill: (providerId: string, enabled: boolean) => void;
  onToggleProviderAutoPrune: (providerId: string, enabled: boolean) => void;
  onToggleProviderPermanentDelete: (providerId: string, enabled: boolean) => void;
  onRequestProviderRefill: (providerId: string) => void;
  onUpdateProviderStoragePassword: (providerId: string, password: string) => boolean;
  requestProviderLifecycleAction: (action: PendingProviderLifecycleAction) => void;
  t: (zh: string, en: string) => string;
};

export function ProviderLifecycleBack({ options,
  poolTargetDrafts, setPoolTargetDrafts, commitPoolTargetDraft, storagePasswordDrafts, setStoragePasswordDrafts,
  editorLocked, onUpdateProviderPoolTargetSize, onToggleProviderAutoRefill, onToggleProviderAutoPrune,
  pruneBusyProviderId, refillBusyProviderId, archivePurgeBusyProviderId,
  onToggleProviderPermanentDelete, onRequestProviderRefill,
  onUpdateProviderStoragePassword, requestProviderLifecycleAction, t,
}: ProviderLifecycleBackProps) {
  const { section, automation, refill } = options;
    const policyKey = `provider:${section.providerId}`;
    const inputValue = poolTargetDrafts[policyKey] ?? String(section.poolTargetSize);
    const archivedCredentialCount = refill?.archivedCredentialCount ?? 0;
    const copyLifecycleValue = async (label: string, value: string | null | undefined) => {
      if (!value) {
        pushAppToast("warning", t(`${label} 暂无可复制内容。`, `${label} is not available to copy.`));
        return;
      }
      if (!navigator.clipboard?.writeText) {
        pushAppToast("warning", t("当前环境不支持剪贴板写入。", "Clipboard access is unavailable."));
        return;
      }
      try {
        await navigator.clipboard.writeText(value);
        pushAppToast("success", t(`已复制${label}。`, `${label} copied.`));
      } catch (cause) {
        pushAppToast("error", cause instanceof Error ? cause.message : t("复制失败。", "Copy failed."));
      }
    };
    const lifecycleAction = (
      kind: PendingProviderLifecycleAction["kind"],
    ): PendingProviderLifecycleAction => ({
      kind,
      providerId: section.providerId,
      providerLabel: section.providerLabel,
      permanentDeleteEnabled: section.permanentDeleteEnabled,
      archivedCredentialCount,
    });

    return (
      <div
        className="nt-provider-lifecycle"
        aria-label={t(
          `${section.providerLabel} 账号生命周期`,
          `${section.providerLabel} credential lifecycle`,
        )}
      >
        <div className="nt-provider-lifecycle__row">
          <div className="nt-provider-lifecycle__metric">
            <UsersRound size={15} aria-hidden="true" />
            <span>{t("可用号池", "Available pool")}</span>
            <span className="nt-provider-lifecycle__capacity">
              <strong>{options.availableCount}/</strong>
              <input
                className="nt-input nt-provider-lifecycle__input"
                type="number"
                min={1}
                inputMode="numeric"
                aria-label={t(
                  `${section.providerLabel} 目标号池容量`,
                  `${section.providerLabel} target pool size`,
                )}
                value={inputValue}
                disabled={editorLocked}
                onChange={(event) => {
                  const nextValue = event.currentTarget.value;
                  setPoolTargetDrafts((current) => ({ ...current, [policyKey]: nextValue }));
                }}
                onBlur={() =>
                  commitPoolTargetDraft(
                    policyKey,
                    section.poolTargetSize,
                    (nextTargetSize) =>
                      onUpdateProviderPoolTargetSize(section.providerId, nextTargetSize),
                  )
                }
                onKeyDown={(event) => {
                  if (event.key === "Enter") {
                    event.currentTarget.blur();
                  } else if (event.key === "Escape") {
                    setPoolTargetDrafts((current) => {
                      const next = { ...current };
                      delete next[policyKey];
                      return next;
                    });
                    event.currentTarget.blur();
                  }
                }}
              />
            </span>
          </div>
          <div className="nt-provider-lifecycle__actions">
            <span className="nt-provider-lifecycle__toggle">
              <span>{t("自动补号", "Auto refill")}</span>
              <button
                className={section.autoRefillEnabled ? "nt-switch nt-switch--on" : "nt-switch"}
                type="button"
                role="switch"
                aria-checked={section.autoRefillEnabled}
                aria-label={t(
                  `${section.providerLabel} 自动补号`,
                  `${section.providerLabel} auto refill`,
                )}
                title={t("自动补号", "Auto refill")}
                disabled={editorLocked}
                onClick={() =>
                  onToggleProviderAutoRefill(section.providerId, !section.autoRefillEnabled)
                }
              >
                <span className="nt-switch__track" aria-hidden="true">
                  <span className="nt-switch__thumb" />
                </span>
              </button>
            </span>
            <button
              className="nt-btn nt-btn--secondary nt-btn--compact nt-provider-lifecycle__command"
              type="button"
              aria-label={t(
                `${section.providerLabel} 手动补号`,
                `Manually refill ${section.providerLabel}`,
              )}
              title={t("投递一次补号任务", "Publish one refill task")}
              disabled={editorLocked || !refill?.userRequestEnabled || refillBusyProviderId === section.providerId}
              onClick={() => onRequestProviderRefill(section.providerId)}
            >
              <UserPlus size={14} aria-hidden="true" />
              <span className="nt-provider-lifecycle__command-label">
                {refillBusyProviderId === section.providerId
                  ? t("补号中", "Refilling")
                  : t("补号", "Refill")}
              </span>
            </button>
          </div>
        </div>

        <div className="nt-provider-lifecycle__row nt-provider-lifecycle__row--quiet">
          <div className="nt-provider-lifecycle__metric">
            <Snowflake size={15} aria-hidden="true" />
            <span>{t("冷却池", "Cooling pool")}</span>
            <strong>{options.coolingCount}</strong>
          </div>
        </div>

        <div className="nt-provider-lifecycle__row">
          <div className="nt-provider-lifecycle__metric nt-provider-lifecycle__metric--danger">
            <ShieldX size={15} aria-hidden="true" />
            <span>{t("失效号", "Invalid")}</span>
            <strong>{options.invalidCount}</strong>
          </div>
          <div className="nt-provider-lifecycle__actions">
            <span className="nt-provider-lifecycle__toggle">
              <span>{t("自动删除", "Auto delete")}</span>
              <button
                className={section.autoPruneEnabled ? "nt-switch nt-switch--on" : "nt-switch"}
                type="button"
                role="switch"
                aria-checked={section.autoPruneEnabled}
                aria-label={t(
                  `${section.providerLabel} 自动删除失效号`,
                  `${section.providerLabel} auto delete invalid credentials`,
                )}
                title={t("自动删除失效号", "Automatically delete invalid credentials")}
                disabled={editorLocked}
                onClick={() =>
                  onToggleProviderAutoPrune(section.providerId, !section.autoPruneEnabled)
                }
              >
                <span className="nt-switch__track" aria-hidden="true">
                  <span className="nt-switch__thumb" />
                </span>
              </button>
            </span>
            <button
              className="nt-btn nt-btn--danger nt-btn--compact nt-provider-lifecycle__command"
              type="button"
              aria-label={t(
                `${section.providerLabel} 手动删除失效号`,
                `Manually delete invalid ${section.providerLabel} credentials`,
              )}
              title={
                section.permanentDeleteEnabled
                  ? t("识别并彻底删除失效号", "Identify and permanently delete invalid credentials")
                  : t("识别失效号并移入归档", "Identify invalid credentials and move them to archive")
              }
              disabled={editorLocked || !automation?.driverConfigured || pruneBusyProviderId === section.providerId}
              onClick={() => requestProviderLifecycleAction(lifecycleAction("prune"))}
            >
              <UserMinus size={14} aria-hidden="true" />
              <span className="nt-provider-lifecycle__command-label">
                {pruneBusyProviderId === section.providerId
                  ? t("删除中", "Deleting")
                  : t("删除", "Delete")}
              </span>
            </button>
          </div>
        </div>

        <div className="nt-provider-lifecycle__archive">
          <div
            className="nt-provider-lifecycle__archive-path"
            title={t("账号归档存储路径", "Account archive storage path")}
          >
            <Archive size={15} aria-hidden="true" />
            <span>{t("账号归档存储路径", "Account archive")}</span>
            <code title={refill?.archiveStoragePath ?? undefined}>{refill?.archiveStoragePath ?? t("未配置", "Not configured")}</code>
            <strong title={t("当前归档账号数", "Archived credential count")}>{archivedCredentialCount}</strong>
            <button
              className="nt-icon-action nt-provider-lifecycle__copy"
              type="button"
              aria-label={t(
                `复制 ${section.providerLabel} 账号归档存储路径`,
                `Copy ${section.providerLabel} account archive path`,
              )}
              title={t("复制账号归档存储路径", "Copy account archive path")}
              disabled={!refill?.archiveStoragePath}
              onClick={() => void copyLifecycleValue(t("账号归档存储路径", "Account archive path"), refill?.archiveStoragePath)}
            >
              <Copy size={13} aria-hidden="true" />
            </button>
          </div>
          <div className="nt-provider-lifecycle__archive-actions">
            <span className="nt-provider-lifecycle__toggle nt-provider-lifecycle__toggle--danger">
              <span>{t("彻底删除", "Permanent delete")}</span>
              <button
                className={section.permanentDeleteEnabled ? "nt-switch nt-switch--danger nt-switch--on" : "nt-switch nt-switch--danger"}
                type="button"
                role="switch"
                aria-checked={section.permanentDeleteEnabled}
                aria-label={t(
                  `${section.providerLabel} 彻底删除模式`,
                  `${section.providerLabel} permanent deletion mode`,
                )}
                title={t("彻底删除模式", "Permanent deletion mode")}
                disabled={editorLocked}
                onClick={() => {
                  if (section.permanentDeleteEnabled) {
                    onToggleProviderPermanentDelete(section.providerId, false);
                  } else {
                    requestProviderLifecycleAction(lifecycleAction("enable-permanent-delete"));
                  }
                }}
              >
                <span className="nt-switch__track" aria-hidden="true">
                  <span className="nt-switch__thumb" />
                </span>
              </button>
            </span>
            <button
              className="nt-btn nt-btn--danger nt-btn--compact nt-provider-lifecycle__command"
              type="button"
              aria-label={t(
                `${section.providerLabel} 手动清空账号归档`,
                `Manually purge the ${section.providerLabel} account archive`,
              )}
              title={t("彻底删除当前归档目录中的账号", "Permanently delete credentials in this archive")}
              disabled={editorLocked || archivedCredentialCount === 0 || archivePurgeBusyProviderId === section.providerId}
              onClick={() => requestProviderLifecycleAction(lifecycleAction("purge-archive"))}
            >
              <ArchiveX size={14} aria-hidden="true" />
              <span className="nt-provider-lifecycle__command-label">
                {archivePurgeBusyProviderId === section.providerId
                  ? t("清理中", "Purging")
                  : t("清空归档", "Purge archive")}
              </span>
            </button>
          </div>
        </div>

        <ProviderStorageEndpoints
          section={section}
          refill={refill}
          storagePasswordDrafts={storagePasswordDrafts}
          setStoragePasswordDrafts={setStoragePasswordDrafts}
          editorLocked={editorLocked}
          onUpdateProviderStoragePassword={onUpdateProviderStoragePassword}
          t={t}
        />
      </div>
    );
}
