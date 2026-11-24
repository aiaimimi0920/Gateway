import { Button } from "@neuro/ui";
import { Building2, UserPlus } from "lucide-react";

type ConsoleWorkspaceHeaderActionsProps = {
  activeWorkspace: string;
  actionBusy: "save" | null;
  autosavePending: boolean;
  draftDirty: boolean;
  editorLocked: boolean;
  accountLedgerProviderOptions: readonly unknown[];
  modelPoolDialogProviderOptions: readonly unknown[];
  setProviderCatalogDialogOpen: (open: boolean) => void;
  openAddCredentialDialog: () => void;
  addAccountGroupRow: () => void;
  openAddModelDialog: () => void;
  t: (zh: string, en: string) => string;
};

export function ConsoleWorkspaceHeaderActions({
  activeWorkspace,
  actionBusy,
  autosavePending,
  draftDirty,
  editorLocked,
  accountLedgerProviderOptions,
  modelPoolDialogProviderOptions,
  setProviderCatalogDialogOpen,
  openAddCredentialDialog,
  addAccountGroupRow,
  openAddModelDialog,
  t,
}: ConsoleWorkspaceHeaderActionsProps) {
  // Keep actionable save states visible without a permanent success label.
  const draftAutosaveStatus = actionBusy === "save" || autosavePending || draftDirty ? (
    <span className="nt-console-autosave" role="status">
      {actionBusy === "save"
        ? t("自动保存中...", "Autosaving...")
        : autosavePending
          ? t("待自动保存", "Autosave pending")
          : t("更改未应用", "Changes not applied")}
    </span>
  ) : null;
  const addProviderLabel = t("添加服务商", "Add provider");
  const addAccountLabel = t("添加账号", "Add account");
  const workspaceHeaderActions =
    activeWorkspace === "accounts" ? (
      <>
        <Button variant="primary" compact className="nt-credential-pool-header__action"
          aria-label={addProviderLabel} title={addProviderLabel} disabled={editorLocked}
          onClick={() => setProviderCatalogDialogOpen(true)}
        >
          <Building2 size={15} aria-hidden="true" />
          <span className="nt-credential-pool-header__action-label">{addProviderLabel}</span>
        </Button>
        <Button compact className="nt-credential-pool-header__action"
          aria-label={addAccountLabel} title={addAccountLabel}
          disabled={editorLocked || accountLedgerProviderOptions.length <= 1}
          onClick={() => openAddCredentialDialog()}
        >
          <UserPlus size={15} aria-hidden="true" />
          <span className="nt-credential-pool-header__action-label">{addAccountLabel}</span>
        </Button>
      </>
    ) : activeWorkspace === "groups" ? (
      <>
        <Button variant="primary"
          disabled={editorLocked}
          onClick={() => addAccountGroupRow()}
        >
          {t("添加分组", "Add group")}
        </Button>
      </>
    ) : activeWorkspace === "models" ? (
      <>
        <Button variant="primary"
          disabled={editorLocked || modelPoolDialogProviderOptions.length === 0}
          onClick={openAddModelDialog}
        >
          {t("添加模型", "Add model")}
        </Button>
        {draftAutosaveStatus}
      </>
    ) : null;

  return workspaceHeaderActions;
}
