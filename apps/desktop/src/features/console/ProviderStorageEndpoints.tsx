import { Check, Copy, Database, KeyRound, Pencil, Search, Webhook, X } from "lucide-react";
import type { Dispatch, SetStateAction } from "react";
import type { ConsoleCredentialRefillDemand } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";

export type ProviderStoragePasswordDraft = { editing: boolean; value: string };
type ProviderStorageEndpointsProps = {
  section: { providerId: string; providerLabel: string };
  refill?: ConsoleCredentialRefillDemand;
  storagePasswordDrafts: Readonly<Record<string, ProviderStoragePasswordDraft>>;
  setStoragePasswordDrafts: Dispatch<SetStateAction<Record<string, ProviderStoragePasswordDraft>>>;
  editorLocked: boolean;
  onUpdateProviderStoragePassword: (providerId: string, password: string) => boolean;
  t: (zh: string, en: string) => string;
};

export function ProviderStorageEndpoints({
  section,
  refill,
  storagePasswordDrafts,
  setStoragePasswordDrafts,
  editorLocked,
  onUpdateProviderStoragePassword,
  t,
}: ProviderStorageEndpointsProps) {
  const storagePasswordDraft = storagePasswordDrafts[section.providerId];
  const storagePasswordConfigured =
  refill?.storagePasswordConfigured === true || Boolean(storagePasswordDraft?.value);
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
      pushAppToast(
        "error",
        cause instanceof Error ? cause.message : t("复制失败。", "Copy failed."),
      );
    }
  };

  return (
    <dl className="nt-provider-lifecycle__endpoints">
      <div>
        <dt title={t("外部补号程序领取补号任务的地址", "Endpoint used by refill workers to claim tasks") }>
          <Webhook size={14} aria-hidden="true" />
          <span>{t("补号通知 API", "Refill API")}</span>
        </dt>
        <dd>
          <code title={refill?.notificationApi}>{refill?.notificationApi ?? "—"}</code>
          <button
            className="nt-icon-action nt-provider-lifecycle__copy"
            type="button"
            aria-label={t(
              `复制 ${section.providerLabel} 补号通知 API`,
              `Copy ${section.providerLabel} refill API`,
            )}
            title={t("复制补号通知 API", "Copy refill API")}
            disabled={!refill?.notificationApi}
            onClick={() => void copyLifecycleValue(t("补号通知 API", "Refill API"), refill?.notificationApi)}
          >
            <Copy size={13} aria-hidden="true" />
          </button>
        </dd>
      </div>
      <div>
        <dt title={t("外部补号程序查询号池缺口的地址", "Endpoint used by refill workers to query pool demand") }>
          <Search size={14} aria-hidden="true" />
          <span>{t("信息查询 API", "Inquiry API")}</span>
        </dt>
        <dd>
          <code title={refill?.inquiryApi}>{refill?.inquiryApi ?? "—"}</code>
          <button
            className="nt-icon-action nt-provider-lifecycle__copy"
            type="button"
            aria-label={t(
              `复制 ${section.providerLabel} 信息查询 API`,
              `Copy ${section.providerLabel} inquiry API`,
            )}
            title={t("复制信息查询 API", "Copy inquiry API")}
            disabled={!refill?.inquiryApi}
            onClick={() => void copyLifecycleValue(t("信息查询 API", "Inquiry API"), refill?.inquiryApi)}
          >
            <Copy size={13} aria-hidden="true" />
          </button>
        </dd>
      </div>
      <div>
        <dt>
          <Database size={14} aria-hidden="true" />
          <span>{t("号码存储路径", "Credential storage")}</span>
        </dt>
        <dd>
          <code title={refill?.credentialStoragePath ?? undefined}>{refill?.credentialStoragePath ?? t("未配置", "Not configured")}</code>
          <button
            className="nt-icon-action nt-provider-lifecycle__copy"
            type="button"
            aria-label={t(
              `复制 ${section.providerLabel} 号码存储路径`,
              `Copy ${section.providerLabel} credential storage path`,
            )}
            title={t("复制号码存储路径", "Copy credential storage path")}
            disabled={!refill?.credentialStoragePath}
            onClick={() => void copyLifecycleValue(t("号码存储路径", "Credential storage path"), refill?.credentialStoragePath)}
          >
            <Copy size={13} aria-hidden="true" />
          </button>
        </dd>
      </div>
      <div>
        <dt title={t("密码按服务商独立保存为敏感字段", "The password is stored per provider as a protected secret") }>
          <KeyRound size={14} aria-hidden="true" />
          <span>{t("存储密码", "Storage password")}</span>
        </dt>
        <dd className="nt-provider-lifecycle__password">
          {storagePasswordDraft?.editing ? (
            <>
              <input
                className="nt-input nt-provider-lifecycle__password-input"
                type="password"
                autoComplete="new-password"
                aria-label={t(
                  `${section.providerLabel} 存储密码`,
                  `${section.providerLabel} storage password`,
                )}
                value={storagePasswordDraft.value}
                disabled={editorLocked}
                onChange={(event) => {
                  const value = event.currentTarget.value;
                  setStoragePasswordDrafts((current) => ({
                    ...current,
                    [section.providerId]: { editing: true, value },
                  }));
                }}
                onKeyDown={(event) => {
                  if (event.key === "Escape") {
                    setStoragePasswordDrafts((current) => ({
                      ...current,
                      [section.providerId]: {
                        editing: false,
                        value: current[section.providerId]?.value ?? "",
                      },
                    }));
                  }
                }}
              />
              <button
                className="nt-icon-action nt-provider-lifecycle__copy"
                type="button"
                aria-label={t(
                  `保存 ${section.providerLabel} 存储密码`,
                  `Save ${section.providerLabel} storage password`,
                )}
                title={t("保存到当前路由草稿", "Save to the current route draft")}
                disabled={editorLocked || storagePasswordDraft.value.length === 0}
                onClick={() => {
                  if (
                    onUpdateProviderStoragePassword(
                      section.providerId,
                      storagePasswordDraft.value,
                    )
                  ) {
                    setStoragePasswordDrafts((current) => ({
                      ...current,
                      [section.providerId]: {
                        editing: false,
                        value: storagePasswordDraft.value,
                      },
                    }));
                  }
                }}
              >
                <Check size={13} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-provider-lifecycle__copy"
                type="button"
                aria-label={t(
                  `取消编辑 ${section.providerLabel} 存储密码`,
                  `Cancel editing ${section.providerLabel} storage password`,
                )}
                title={t("取消编辑", "Cancel editing")}
                onClick={() =>
                  setStoragePasswordDrafts((current) => ({
                    ...current,
                    [section.providerId]: {
                      editing: false,
                      value: current[section.providerId]?.value ?? "",
                    },
                  }))
                }
              >
                <X size={13} aria-hidden="true" />
              </button>
            </>
          ) : (
            <>
              <code title={storagePasswordConfigured ? t("已配置", "Configured") : t("未设置", "Not set")}>
                {storagePasswordConfigured ? "••••••••" : "—"}
              </code>
              <button
                className="nt-icon-action nt-provider-lifecycle__copy"
                type="button"
                aria-label={t(
                  `编辑 ${section.providerLabel} 存储密码`,
                  `Edit ${section.providerLabel} storage password`,
                )}
                title={t("编辑存储密码", "Edit storage password")}
                disabled={editorLocked}
                onClick={() =>
                  setStoragePasswordDrafts((current) => ({
                    ...current,
                    [section.providerId]: {
                      editing: true,
                      value: current[section.providerId]?.value ?? "",
                    },
                  }))
                }
              >
                <Pencil size={13} aria-hidden="true" />
              </button>
              <button
                className="nt-icon-action nt-provider-lifecycle__copy"
                type="button"
                aria-label={t(
                  `复制 ${section.providerLabel} 存储密码`,
                  `Copy ${section.providerLabel} storage password`,
                )}
                title={
                  storagePasswordDraft?.value
                    ? t("复制本次输入的存储密码", "Copy the storage password entered this session")
                    : t("已保存密码不会回传；编辑后可复制本次输入", "Saved passwords are not returned; edit one to copy the current input")
                }
                disabled={!storagePasswordDraft?.value}
                onClick={() => void copyLifecycleValue(t("存储密码", "Storage password"), storagePasswordDraft?.value)}
              >
                <Copy size={13} aria-hidden="true" />
              </button>
            </>
          )}
        </dd>
      </div>
    </dl>
  );
}
