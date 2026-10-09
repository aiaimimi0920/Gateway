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
  // Workspace actions live in the shell board header so no workspace needs a
  // third command bar of its own.
  const workspaceHeaderActions =
    activeWorkspace === "accounts" ? (
      <>
        <button
          className="nt-btn nt-btn--primary"
          type="button"
          disabled={editorLocked}
          onClick={() => setProviderCatalogDialogOpen(true)}
        >
          {t("添加服务商", "Add provider")}
        </button>
        <button
          className="nt-btn nt-btn--outline"
          type="button"
          disabled={editorLocked || accountLedgerProviderOptions.length <= 1}
          onClick={() => openAddCredentialDialog()}
        >
          {t("添加账号", "Add account")}
        </button>
      </>
    ) : activeWorkspace === "groups" ? (
      <>
        <button
          className="nt-btn nt-btn--primary"
          type="button"
          disabled={editorLocked}
          onClick={() => addAccountGroupRow()}
        >
          {t("添加分组", "Add group")}
        </button>
      </>
    ) : activeWorkspace === "models" ? (
      <>
        <button
          className="nt-btn nt-btn--primary"
          type="button"
          disabled={editorLocked || modelPoolDialogProviderOptions.length === 0}
          onClick={openAddModelDialog}
        >
          {t("添加模型", "Add model")}
        </button>
        {draftAutosaveStatus}
      </>
    ) : null;

  return workspaceHeaderActions;
}
