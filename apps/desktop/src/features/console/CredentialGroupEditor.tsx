import type { Ref } from "react";

import type { AccountGroupDraftLike } from "./accountManagementViewModel";
import type { CredentialGroupsWorkspaceProps, TranslateFn } from "./credentialGroupsWorkspaceTypes";

type CredentialGroupEditorProps = Pick<
  CredentialGroupsWorkspaceProps,
  | "editorLocked"
  | "selectedGroupIdInvalid"
  | "selectedGroupBillingInvalid"
  | "onUpdateField"
  | "onToggleEnabled"
  | "onRemoveGroup"
> & {
  t: TranslateFn;
  group: AccountGroupDraftLike;
  nameInputRef?: Ref<HTMLInputElement>;
  groupIdInputRef?: Ref<HTMLInputElement>;
  creating?: boolean;
  groupIdError?: string;
};

function groupTitle(t: TranslateFn, group: AccountGroupDraftLike): string {
  return group.name.trim() || group.groupId.trim() || t("新分组", "New group");
}

export function CredentialGroupEditor({
  t,
  group,
  nameInputRef,
  groupIdInputRef,
  creating = false,
  groupIdError,
  editorLocked,
  selectedGroupIdInvalid,
  selectedGroupBillingInvalid,
  onUpdateField,
  onToggleEnabled,
  onRemoveGroup,
}: CredentialGroupEditorProps) {
  const title = groupTitle(t, group);
  const groupIdErrorId = `${group.id}-group-id-error`;
  const billingErrorId = `${group.id}-billing-multiplier-error`;

  return (
    <article className="nt-card nt-card--panel nt-group-panel nt-group-panel--editor">
      <div className="nt-section__head">
        <div>
          <h3>{title}</h3>
        </div>
        <div className="nt-actions">
          <label className="nt-chip">
            <input
              type="checkbox"
              checked={group.enabled}
              disabled={editorLocked}
              aria-label={t(`${title} 启用状态`, `${title} enabled state`)}
              onChange={(event) => onToggleEnabled(group.id, event.currentTarget.checked)}
            />
            <span>{group.enabled ? t("启用", "Enabled") : t("停用", "Disabled")}</span>
          </label>
          {!creating && <button
            className="nt-btn nt-btn--outline"
            type="button"
            disabled={editorLocked}
            onClick={() => onRemoveGroup(group.id)}
          >
            {t("移除分组", "Remove group")}
          </button>}
        </div>
      </div>

      <div className="nt-form-grid">
        <label className="nt-field">
          <span id={`${group.id}-group-id-label`}>{t("分组 ID", "Group ID")}</span>
          <input
            ref={groupIdInputRef}
            aria-labelledby={`${group.id}-group-id-label`}
            className="nt-input"
            value={group.groupId}
            disabled={editorLocked}
            aria-invalid={selectedGroupIdInvalid}
            aria-describedby={selectedGroupIdInvalid ? groupIdErrorId : undefined}
            onChange={(event) => onUpdateField(group.id, "groupId", event.currentTarget.value)}
          />
          {selectedGroupIdInvalid ? (
            <small id={groupIdErrorId}>{groupIdError ?? t("分组 ID 必须填写。", "Group ID is required.")}</small>
          ) : null}
        </label>
        <label className="nt-field">
          <span>{t("分组名称", "Group name")}</span>
          <input
            ref={nameInputRef}
            className="nt-input"
            value={group.name}
            disabled={editorLocked}
            onChange={(event) => onUpdateField(group.id, "name", event.currentTarget.value)}
          />
        </label>
        <label className="nt-field">
          <span id={`${group.id}-billing-label`}>{t("计费倍率", "Billing multiplier")}</span>
          <input
            className="nt-input"
            inputMode="decimal"
            aria-labelledby={`${group.id}-billing-label`}
            value={group.billingMultiplier}
            disabled={editorLocked}
            aria-invalid={selectedGroupBillingInvalid}
            aria-describedby={selectedGroupBillingInvalid ? billingErrorId : undefined}
            onChange={(event) =>
              onUpdateField(group.id, "billingMultiplier", event.currentTarget.value)
            }
          />
          {selectedGroupBillingInvalid ? (
            <small id={billingErrorId}>
              {t(
                "分组计费倍率必须是大于等于 0 的数字。",
                "Group billing multiplier must be a number greater than or equal to 0.",
              )}
            </small>
          ) : null}
        </label>
        <label className="nt-field nt-field--wide">
          <span>{t("描述", "Description")}</span>
          <input
            className="nt-input"
            value={group.description}
            disabled={editorLocked}
            onChange={(event) => onUpdateField(group.id, "description", event.currentTarget.value)}
          />
        </label>
        <label className="nt-field nt-field--wide">
          <span>{t("备注", "Notes")}</span>
          <textarea
            className="nt-input nt-textarea"
            value={group.notes}
            disabled={editorLocked}
            onChange={(event) => onUpdateField(group.id, "notes", event.currentTarget.value)}
          />
        </label>
      </div>
    </article>
  );
}
