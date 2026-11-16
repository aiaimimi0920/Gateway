import { Button, CheckboxGroup, Field, Input, Notice, Select } from "@neuro/ui";
import type { RefObject } from "react";
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
import "./accessKeyDialogs.css";

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
  nameInputRef: RefObject<HTMLInputElement | null>;
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
  nameInputRef,
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
      className="nt-key-editor"
      onSubmit={(event) => {
        event.preventDefault();
        if (!busy && !locked && valid) onSubmit();
      }}
    >
      <div className="nt-key-dialog__body">
        <fieldset className="nt-key-form" disabled={busy || locked}>
          <Field label={t("名称", "Name")}>
            <Input
              ref={nameInputRef}
              required
              maxLength={128}
              value={draft.displayName}
              placeholder={t("例如：我的客户端", "e.g. My client")}
              onChange={(event) => patch({ displayName: event.target.value })}
            />
          </Field>
          <Field label={t("有效期", "Validity")}>
            <Select
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
            </Select>
          </Field>
          <AccessKeyGroupPicker
            groups={groups}
            selected={draft.accountGroupIds}
            onChange={(accountGroupIds) => patch({ accountGroupIds })}
            disabled={busy || locked}
            t={t}
          />
          <AccessKeyQuotaFields
            draft={draft}
            onChange={patch}
            balance={balance}
            cashQuotaSupported={cashQuotaSupported}
            t={t}
          />
          {!local ? (
            <>
              {(
                [
                  ["ownerType", t("归属类型", "Owner type")],
                  ["ownerId", t("归属 ID", "Owner ID")],
                  ["resolvedProjectId", t("项目 ID", "Project ID")],
                  ["resolvedTenantId", t("租户 ID", "Tenant ID")],
                ] as const
              ).map(([field, label]) => (
                <Field label={label} key={field}>
                  <Input
                    required
                    maxLength={128}
                    disabled={
                      editing &&
                      (field === "resolvedProjectId" ||
                        field === "resolvedTenantId")
                    }
                    value={draft[field]}
                    onChange={(event) => patch({ [field]: event.target.value })}
                  />
                </Field>
              ))}
              <Field label={t("密钥类型", "Key kind")}>
                <Select
                  value={draft.keyKind}
                  disabled={editing}
                  onChange={(event) => patch({ keyKind: event.target.value })}
                >
                  <option value="normal">{t("普通", "Normal")}</option>
                  <option value="aggregate">{t("聚合", "Aggregate")}</option>
                </Select>
              </Field>
            </>
          ) : null}
          {bundles.length > 0 ? (
            <CheckboxGroup label={t("权益包", "Bundles")} orientation="vertical"
              options={bundles.map(bundle => ({ value: bundle.id, label: bundle.displayName || bundle.slug }))}
              values={draft.bundleIds} onValuesChange={bundleIds => patch({ bundleIds })} />
          ) : null}
        </fieldset>
      </div>
      <footer className="nt-key-dialog__footer">
        {error ? (
          <Notice tone="danger" tabIndex={0} title={error}>
            {error}
          </Notice>
        ) : null}
        <div className="n-dialog-actions">
          {onCancel ? (
            <Button
              type="button"
              disabled={busy || locked}
              onClick={onCancel}
            >
              {t("取消", "Cancel")}
            </Button>
          ) : null}
          <Button
            variant="primary"
            loading={busy}
            type="submit"
            disabled={busy || locked || !valid}
          >
            {editing ? t("保存", "Save") : t("创建 API Key", "Create API Key")}
          </Button>
        </div>
      </footer>
    </form>
  );
}
