import * as Dialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
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
  onRestoreFocus(): void;
  onClose(): void;
  open?: boolean;
  onCreate?: FormEventHandler<HTMLFormElement>;
  onCaptureFocus?: () => void;
  groupIdError?: string;
};

// This modal only owns presentation; field/member changes retain the route autosave owner.
export function CredentialGroupEditDialog({
  t, notice, error, group, editorLocked, selectedGroupIdInvalid,
  selectedGroupBillingInvalid, memberCandidates, memberQuery, memberMode,
  onUpdateField, onToggleEnabled, onRemoveGroup, onMemberQueryChange,
  onMemberModeChange, onToggleMember, onRestoreFocus, onClose,
  open = true, onCreate, onCaptureFocus, groupIdError,
}: CredentialGroupEditDialogProps) {
  const nameInputRef = useRef<HTMLInputElement>(null);
  const groupIdInputRef = useRef<HTMLInputElement>(null);

  return (
    <Dialog.Root open={open} onOpenChange={(open) => { if (!open) onClose(); }}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          asChild
          className="dialog-content nt-group-edit-dialog"
          onOpenAutoFocus={(event) => {
            onCaptureFocus?.();
            if (!editorLocked) {
              event.preventDefault();
              (onCreate ? groupIdInputRef : nameInputRef).current?.focus();
            }
          }}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            onRestoreFocus();
          }}
        >
          <form onSubmit={(event) => { event.preventDefault(); onCreate?.(event); }} noValidate>
          <div className="nt-group-edit-dialog__head">
            <Dialog.Title>{onCreate ? t("添加分组", "Add group") : t("编辑权益组", "Edit entitlement group")}</Dialog.Title>
            <Dialog.Close asChild>
              <button className="nt-icon-action" type="button" aria-label={t("关闭", "Close")}>
                <X size={16} aria-hidden="true" />
              </button>
            </Dialog.Close>
          </div>
          <Dialog.Description className="nt-visually-hidden">
            {onCreate ? t("填写分组信息与成员，确认后创建。", "Set group details and members, then confirm to create.") : t("编辑分组信息与成员，修改自动保存。", "Edit group details and members. Changes save automatically.")}
          </Dialog.Description>
          <div className="nt-group-edit-dialog__body">
            {notice}
            {error ? <div className="nt-alert nt-alert--danger" role="alert">{error}</div> : null}
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
          <div className="dialog-actions">
            <Dialog.Close asChild>
              <button className={onCreate ? "nt-btn nt-btn--outline" : "nt-btn nt-btn--primary"} type="button">{onCreate ? t("取消", "Cancel") : t("完成", "Done")}</button>
            </Dialog.Close>
            {onCreate ? <button className="nt-btn nt-btn--primary" type="submit" disabled={editorLocked}>{t("创建分组", "Create group")}</button> : null}
          </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
