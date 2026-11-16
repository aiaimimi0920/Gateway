import { Button, Dialog, Notice, type DialogProps } from "@neuro/ui";
import { type FormEventHandler, useRef } from "react";

import { CredentialGroupEditor } from "./CredentialGroupEditor";
import { CredentialGroupMembersPanel } from "./CredentialGroupMembersPanel";
import type { CredentialGroupsWorkspaceProps } from "./credentialGroupsWorkspaceTypes";
import "../../styles/credential-group-edit-dialog.css";

type CredentialGroupEditDialogProps = Pick<
  CredentialGroupsWorkspaceProps,
  | "t"
  | "notice"
  | "error"
  | "editorLocked"
  | "selectedGroupIdInvalid"
  | "selectedGroupBillingInvalid"
  | "memberCandidates"
  | "memberQuery"
  | "memberMode"
  | "onUpdateField"
  | "onToggleEnabled"
  | "onRemoveGroup"
  | "onMemberQueryChange"
  | "onMemberModeChange"
  | "onToggleMember"
> & {
  group: NonNullable<CredentialGroupsWorkspaceProps["selectedGroup"]>;
  resolveReturnFocus?: DialogProps["resolveReturnFocus"];
  onClose(): void;
  open?: boolean;
  onCreate?: FormEventHandler<HTMLFormElement>;
  groupIdError?: string;
};

// This modal only owns presentation; field/member changes retain the route autosave owner.
export function CredentialGroupEditDialog({
  t, notice, error, group, editorLocked, selectedGroupIdInvalid,
  selectedGroupBillingInvalid, memberCandidates, memberQuery, memberMode,
  onUpdateField, onToggleEnabled, onRemoveGroup, onMemberQueryChange,
  onMemberModeChange, onToggleMember, resolveReturnFocus, onClose,
  open = true, onCreate, groupIdError,
}: CredentialGroupEditDialogProps) {
  const nameInputRef = useRef<HTMLInputElement>(null);
  const groupIdInputRef = useRef<HTMLInputElement>(null);
  const fieldError = selectedGroupIdInvalid ? groupIdError ?? t("分组 ID 必须填写。", "Group ID is required.")
    : selectedGroupBillingInvalid ? t("分组计费倍率必须是大于等于 0 的数字。", "Group billing multiplier must be a number greater than or equal to 0.") : undefined;

  return (
    <Dialog open={open} onOpenChange={(open) => { if (!open) onClose(); }}
      title={onCreate ? t("添加分组", "Add group") : t("编辑权益组", "Edit entitlement group")}
      closeLabel={t("关闭", "Close")} className="nt-group-edit-dialog"
      initialFocusRef={onCreate ? groupIdInputRef : nameInputRef}
      resolveReturnFocus={resolveReturnFocus}>
      <form onSubmit={(event) => { event.preventDefault(); onCreate?.(event); }} noValidate>
        <div className="nt-group-edit-dialog__body">
          {notice}
          {error && error !== fieldError ? <Notice tone="danger">{error}</Notice> : null}
          <CredentialGroupEditor
            t={t}
            group={group}
            nameInputRef={nameInputRef}
            groupIdInputRef={groupIdInputRef}
            creating={Boolean(onCreate)}
            groupIdError={groupIdError}
            editorLocked={editorLocked}
            selectedGroupIdInvalid={selectedGroupIdInvalid}
            selectedGroupBillingInvalid={selectedGroupBillingInvalid}
            onUpdateField={onUpdateField}
            onToggleEnabled={onToggleEnabled}
            onRemoveGroup={onRemoveGroup}
          />
          <CredentialGroupMembersPanel
            t={t}
            group={group}
            editorLocked={editorLocked}
            candidates={memberCandidates}
            query={memberQuery}
            mode={memberMode}
            onQueryChange={onMemberQueryChange}
            onModeChange={onMemberModeChange}
            onToggleMember={onToggleMember}
          />
        </div>
        <div className="n-dialog-actions">
          <Button variant={onCreate ? "secondary" : "primary"} onClick={onClose}>{onCreate ? t("取消", "Cancel") : t("完成", "Done")}</Button>
          {onCreate ? <Button variant="primary" type="submit" disabled={editorLocked}>{t("创建分组", "Create group")}</Button> : null}
        </div>
      </form>
    </Dialog>
  );
}
