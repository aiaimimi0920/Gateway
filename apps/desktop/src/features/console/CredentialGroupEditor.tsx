import type { Ref } from "react";
import { Button, Field, Input, Switch, Textarea } from "@neuro/ui";

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

  return (
    <section className="nt-group-editor">
      <div className="nt-section__head">
        <div>
          <h3>{title}</h3>
        </div>
        <div className="nt-actions">
          <Switch
            checked={group.enabled}
            disabled={editorLocked}
            aria-label={t(`${title} 启用状态`, `${title} enabled state`)}
            label={group.enabled ? t("启用", "Enabled") : t("停用", "Disabled")}
            onChange={(event) => onToggleEnabled(group.id, event.currentTarget.checked)}
          />
          {!creating && <Button variant="danger" compact
            disabled={editorLocked}
            onClick={() => onRemoveGroup(group.id)}
          >
            {t("移除分组", "Remove group")}
          </Button>}
        </div>
      </div>

      <div className="nt-group-editor__fields">
        <Field label={t("分组 ID", "Group ID")}
          error={selectedGroupIdInvalid ? groupIdError ?? t("分组 ID 必须填写。", "Group ID is required.") : undefined}>
          <Input
            ref={groupIdInputRef}
            value={group.groupId}
            disabled={editorLocked}
            aria-invalid={selectedGroupIdInvalid}
            onChange={(event) => onUpdateField(group.id, "groupId", event.currentTarget.value)}
          />
        </Field>
        <Field label={t("分组名称", "Group name")}>
          <Input
            ref={nameInputRef}
            value={group.name}
            disabled={editorLocked}
            onChange={(event) => onUpdateField(group.id, "name", event.currentTarget.value)}
          />
        </Field>
        <Field label={t("计费倍率", "Billing multiplier")}
          error={selectedGroupBillingInvalid ? t("分组计费倍率必须是大于等于 0 的数字。", "Group billing multiplier must be a number greater than or equal to 0.") : undefined}>
          <Input
            inputMode="decimal"
            value={group.billingMultiplier}
            disabled={editorLocked}
            aria-invalid={selectedGroupBillingInvalid}
            onChange={(event) =>
              onUpdateField(group.id, "billingMultiplier", event.currentTarget.value)
            }
          />
        </Field>
        <div className="nt-group-editor__wide"><Field label={t("描述", "Description")}>
          <Input
            value={group.description}
            disabled={editorLocked}
            onChange={(event) => onUpdateField(group.id, "description", event.currentTarget.value)}
          />
        </Field></div>
        <div className="nt-group-editor__wide"><Field label={t("备注", "Notes")}>
          <Textarea
            value={group.notes}
            disabled={editorLocked}
            onChange={(event) => onUpdateField(group.id, "notes", event.currentTarget.value)}
          />
        </Field></div>
      </div>
    </section>
  );
}
