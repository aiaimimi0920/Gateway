import { ConfirmDialog } from "@neuro/ui";
import { useRef, type ComponentProps, type Dispatch, type ReactNode, type SetStateAction } from "react";
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
  poolView?: ModelWorkspaceProps["poolView"];
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
  poolView,
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
  const workspaceRef = useRef<HTMLDivElement>(null);
  const resolveReturnFocus = (target: HTMLElement | null) =>
    target?.isConnected && target !== document.body && !target.matches(":disabled") && !target.closest("[inert]")
      ? target : workspaceRef.current;
  return (
    <>
      <div ref={workspaceRef} tabIndex={-1} className="nt-model-workspace-frame">
        <ModelPoolWorkspace
          t={t}
          notice={draftStructureNotice}
          editorLocked={editorLocked}
          models={modelPoolDirectory}
          poolView={poolView}
          accountCards={entitlementAccountCardBridge}
          onMoveProvider={reorderModelPoolChain}
          onResetChain={resetModelPoolChain}
          onToggleEnabled={toggleModelPoolEnabled}
          onEditModel={openEditModelDialog}
          onDeleteModel={setPendingModelPoolRemoval}
        />
      </div>
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
        resolveReturnFocus={resolveReturnFocus}
      />
      <ConfirmDialog
        open={pendingModelPoolRemoval !== null}
        onOpenChange={(open) => {
          if (!open) {
            setPendingModelPoolRemoval(null);
          }
        }}
        title={t("确认删除模型", "Confirm model deletion")}
        description={pendingModelPoolRemoval
          ? t(
            `将从当前草稿中删除模型 ${pendingModelPoolRemoval}：它的服务商顺序、各账号的模型声明、别名与模型映射都会一并移除。`,
            `This removes model ${pendingModelPoolRemoval} from the current draft: its provider order, every account declaration, its aliases and its model map entries all go with it.`,
          )
          : ""}
        confirmLabel={t("确认删除", "Delete model")}
        cancelLabel={t("取消", "Cancel")}
        confirmDisabled={editorLocked}
        resolveReturnFocus={resolveReturnFocus}
        onConfirm={() => {
          if (pendingModelPoolRemoval) {
            deleteModelPoolModel(pendingModelPoolRemoval);
          }
          setPendingModelPoolRemoval(null);
        }}
      />
      {groupAccountRemovalDialog}
    </>
  );
}
