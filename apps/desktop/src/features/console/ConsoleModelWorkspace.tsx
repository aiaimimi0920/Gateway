import * as Dialog from "@radix-ui/react-dialog";
import type { ComponentProps, Dispatch, ReactNode, SetStateAction } from "react";
import { ModelPoolWorkspace } from "./ModelPoolWorkspace";
import {
  ModelPoolModelDialog,
  type ModelPoolModelDialogMode,
  type ModelPoolModelDialogValue,
} from "./ModelPoolModelDialog";

type ModelWorkspaceProps = ComponentProps<typeof ModelPoolWorkspace>;
type ModelDialogProps = ComponentProps<typeof ModelPoolModelDialog>;
type ConsoleModelWorkspaceProps = {
  t: ModelWorkspaceProps["t"];
  draftStructureNotice: ModelWorkspaceProps["notice"];
  editorLocked: boolean;
  modelPoolDirectory: ModelWorkspaceProps["models"];
  entitlementAccountCardBridge: ModelWorkspaceProps["accountCards"];
  reorderModelPoolChain: ModelWorkspaceProps["onMoveProvider"];
  resetModelPoolChain: ModelWorkspaceProps["onResetChain"];
  toggleModelPoolEnabled: ModelWorkspaceProps["onToggleEnabled"];
  openEditModelDialog: ModelWorkspaceProps["onEditModel"];
  setPendingModelPoolRemoval: Dispatch<SetStateAction<string | null>>;
  modelPoolDialog: { mode: ModelPoolModelDialogMode; initial: ModelPoolModelDialogValue | null } | null;
  modelPoolDialogProviderOptions: ModelDialogProps["providerOptions"];
  modelPoolModelNames: ModelDialogProps["existingModels"];
  setModelPoolDialog: Dispatch<SetStateAction<{ mode: ModelPoolModelDialogMode; initial: ModelPoolModelDialogValue | null } | null>>;
  submitModelPoolDialog: ModelDialogProps["onSubmit"];
  pendingModelPoolRemoval: string | null;
  deleteModelPoolModel: (model: string) => void;
  groupAccountRemovalDialog: ReactNode;
};

export function ConsoleModelWorkspace({
  t,
  draftStructureNotice,
  editorLocked,
  modelPoolDirectory,
  entitlementAccountCardBridge,
  reorderModelPoolChain,
  resetModelPoolChain,
  toggleModelPoolEnabled,
  openEditModelDialog,
  setPendingModelPoolRemoval,
  modelPoolDialog,
  modelPoolDialogProviderOptions,
  modelPoolModelNames,
  setModelPoolDialog,
  submitModelPoolDialog,
  pendingModelPoolRemoval,
  deleteModelPoolModel,
  groupAccountRemovalDialog,
}: ConsoleModelWorkspaceProps) {
  return (
    <>
      <ModelPoolWorkspace
        t={t}
        notice={draftStructureNotice}
        editorLocked={editorLocked}
        models={modelPoolDirectory}
        accountCards={entitlementAccountCardBridge}
        onMoveProvider={reorderModelPoolChain}
        onResetChain={resetModelPoolChain}
        onToggleEnabled={toggleModelPoolEnabled}
        onEditModel={openEditModelDialog}
        onDeleteModel={setPendingModelPoolRemoval}
      />
      <ModelPoolModelDialog
        open={modelPoolDialog !== null}
        mode={modelPoolDialog?.mode ?? "add"}
        providerOptions={modelPoolDialogProviderOptions}
        existingModels={modelPoolModelNames}
        initialValue={modelPoolDialog?.initial ?? null}
        locked={editorLocked}
        onOpenChange={(open) => {
          if (!open) {
            setModelPoolDialog(null);
          }
        }}
        onSubmit={submitModelPoolDialog}
      />
      <Dialog.Root
        open={pendingModelPoolRemoval !== null}
        onOpenChange={(open) => {
          if (!open) {
            setPendingModelPoolRemoval(null);
          }
        }}
      >
        <Dialog.Portal>
          <Dialog.Overlay className="dialog-overlay" />
          <Dialog.Content
            className="dialog-content nt-credential-remove-dialog"
            aria-describedby="model-pool-remove-description"
          >
            <Dialog.Title>{t("确认删除模型", "Confirm model deletion")}</Dialog.Title>
            <Dialog.Description id="model-pool-remove-description">
              {pendingModelPoolRemoval
                ? t(
                    `将从当前草稿中删除模型 ${pendingModelPoolRemoval}：它的服务商顺序、各账号的模型声明、别名与模型映射都会一并移除。`,
                    `This removes model ${pendingModelPoolRemoval} from the current draft: its provider order, every account declaration, its aliases and its model map entries all go with it.`,
                  )
                : ""}
            </Dialog.Description>
            <div className="nt-validation-list nt-validation-list--warning">
              <strong>{t("请再次确认", "Please confirm")}</strong>
              <ul>
                <li>{t("其他模型不会受到影响。", "Other models are not affected.")}</li>
              </ul>
            </div>
            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button">{t("取消", "Cancel")}</button>
              </Dialog.Close>
              <button
                className="nt-btn nt-btn--danger"
                type="button"
                onClick={() => {
                  if (pendingModelPoolRemoval) {
                    deleteModelPoolModel(pendingModelPoolRemoval);
                  }
                  setPendingModelPoolRemoval(null);
                }}
              >
                {t("确认删除", "Delete model")}
              </button>
            </div>
          </Dialog.Content>
        </Dialog.Portal>
      </Dialog.Root>
      {groupAccountRemovalDialog}
    </>
  );
}
