import type {
  ConsoleAccessBundle,
  ConsoleAccessKeyGroup,
  ConsoleAccessKeyBalance,
} from "../../api/contracts";
import { AccessKeyGroupPicker } from "./AccessKeyGroupPicker";
import { validKeyGroups } from "./accessKeyGroups";
import { accessKeyDraftValid } from "./accessKeyPresentation";
import type { AccessKeyDraft, TranslateFn } from "./accessKeysTypes";
import { AccessKeyQuotaFields } from "./AccessKeyQuotaFields";
import { validKeyQuota } from "./accessKeyQuota";

type Props = {
  draft: AccessKeyDraft;
  onChange(draft: AccessKeyDraft): void;
  onSubmit(): void;
  busy: boolean;
  locked: boolean;
  local: boolean;
  bundles: ConsoleAccessBundle[];
  groups: ConsoleAccessKeyGroup[] | undefined;
  error: string | null;
  t: TranslateFn;
  editing?: boolean;
  balance?: ConsoleAccessKeyBalance;
  cashQuotaSupported?: boolean;
  onCancel?(): void;
};

export function AccessKeyCreateForm({
  draft,
  onChange,
  onSubmit,
  busy,
  locked,
  local,
  bundles,
  groups,
  error,
  t,
  editing = false,
  balance,
  cashQuotaSupported = false,
  onCancel,
}: Props) {
  const patch = (value: Partial<AccessKeyDraft>) =>
    onChange({ ...draft, ...value });
  const valid =
    accessKeyDraftValid(draft) &&
    validKeyGroups(draft.accountGroupIds, groups) &&
    validKeyQuota(draft) &&
    (draft.quotaMode !== "cash_prepaid" || cashQuotaSupported);
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        if (!busy && !locked && valid) onSubmit();
      }}
    >
      <fieldset className="nt-key-form" disabled={busy || locked}>
        <label className="nt-field">
          <span>{t("名称", "Name")}</span>
          <input
            className="nt-input"
            autoFocus
            required
            maxLength={128}
            value={draft.displayName}
            placeholder={t("例如：我的客户端", "e.g. My client")}
            onChange={(event) => patch({ displayName: event.target.value })}
          />
        </label>
        <AccessKeyGroupPicker
          groups={groups}
          selected={draft.accountGroupIds}
          onChange={(accountGroupIds) => patch({ accountGroupIds })}
          disabled={busy || locked}
          t={t}
        />
        <label className="nt-field">
          <span>{t("有效期", "Validity")}</span>
          <select
            className="nt-input"
            value={draft.expiresAt ? "current" : "0"}
            onChange={(event) => {
              const days = Number(event.target.value);
              patch({
                expiresAt:
                  days === 0
                    ? ""
                    : new Date(Date.now() + days * 86_400_000).toISOString(),
              });
            }}
          >
            {draft.expiresAt ? (
              <option value="current" disabled>
                {new Date(draft.expiresAt).toLocaleString()}
              </option>
            ) : null}
            <option value="0">{t("永不过期", "Never expires")}</option>
            <option value="7">{t("7 天", "7 days")}</option>
            <option value="30">{t("30 天", "30 days")}</option>
            <option value="90">{t("90 天", "90 days")}</option>
          </select>
        </label>
        <AccessKeyQuotaFields
          draft={draft}
          onChange={patch}
          balance={balance}
          cashQuotaSupported={cashQuotaSupported}
          t={t}
        />
        <details
          className="nt-key-advanced-fields"
          open={local ? undefined : true}
        >
          <summary>
            {t("归属与高级设置", "Ownership and advanced settings")}
          </summary>
          <div className="nt-console-field-grid">
            {(
              [
                ["ownerType", t("归属类型", "Owner type")],
                ["ownerId", t("归属 ID", "Owner ID")],
                ["resolvedProjectId", t("项目 ID", "Project ID")],
                ["resolvedTenantId", t("租户 ID", "Tenant ID")],
                ["publicKeyPrefix", t("密钥前缀", "Key prefix")],
              ] as const
            ).map(([field, label]) => (
              <label className="nt-field" key={field}>
                <span>{label}</span>
                <input
                  className="nt-input"
                  required
                  maxLength={128}
                  disabled={
                    editing &&
                    (field === "resolvedProjectId" ||
                      field === "resolvedTenantId" ||
                      field === "publicKeyPrefix")
                  }
                  value={draft[field]}
                  onChange={(event) => patch({ [field]: event.target.value })}
                />
              </label>
            ))}
            {!local ? (
              <label className="nt-field">
                <span>{t("密钥类型", "Key kind")}</span>
                <select
                  className="nt-input"
                  value={draft.keyKind}
                  disabled={editing}
                  onChange={(event) => patch({ keyKind: event.target.value })}
                >
                  <option value="normal">{t("普通", "Normal")}</option>
                  <option value="aggregate">{t("聚合", "Aggregate")}</option>
                </select>
              </label>
            ) : null}
          </div>
          {bundles.length > 0 ? (
            <fieldset className="nt-key-bundle-options">
              <legend>{t("权益包", "Bundles")}</legend>
              {bundles.map((bundle) => (
                <label key={bundle.id}>
                  <input
                    type="checkbox"
                    checked={draft.bundleIds.includes(bundle.id)}
                    onChange={(event) => {
                      patch({
                        bundleIds: event.target.checked
                          ? [...draft.bundleIds, bundle.id]
                          : draft.bundleIds.filter((id) => id !== bundle.id),
                      });
                    }}
                  />
                  {bundle.displayName || bundle.slug}
                </label>
              ))}
            </fieldset>
          ) : null}
        </details>
      </fieldset>
      {error ? (
        <div className="nt-alert nt-alert--danger" role="alert">
          {error}
        </div>
      ) : null}
      <div className="dialog-actions">
        {onCancel ? (
          <button
            className="nt-btn nt-btn--outline"
            type="button"
            disabled={busy || locked}
            onClick={onCancel}
          >
            {t("取消", "Cancel")}
          </button>
        ) : null}
        <button
          className="nt-btn nt-btn--primary"
          type="submit"
          disabled={busy || locked || !valid}
        >
          {editing
            ? busy
              ? t("保存中…", "Saving…")
              : t("保存", "Save")
            : busy
              ? t("创建中…", "Creating…")
              : t("创建 API Key", "Create API Key")}
        </button>
      </div>
    </form>
  );
}
