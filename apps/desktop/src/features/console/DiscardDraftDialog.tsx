import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type DiscardDraftDialogProps = {
  open: boolean;
  busy: boolean;
  onOpenChange(open: boolean): void;
  onConfirm(): Promise<void>;
};

export function DiscardDraftDialog({
  open,
  busy,
  onOpenChange,
  onConfirm,
}: DiscardDraftDialogProps) {
  const { t } = useUiLocale();

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    await onConfirm();
    onOpenChange(false);
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content" aria-describedby="discard-draft-description">
          <Dialog.Title>{t("丢弃未保存修改", "Discard unsaved changes")}</Dialog.Title>
          <Dialog.Description id="discard-draft-description">
            {t(
              "刷新会从 Gateway 重新加载当前激活修订，并丢弃尚未保存的路由、账号、分组和敏感字段草稿。",
              "Refreshing reloads the active Gateway revision and discards unsaved route, account, group, and secret drafts.",
            )}
          </Dialog.Description>
          <form className="nt-stack" onSubmit={(event) => void submit(event)}>
            <div className="nt-validation-list nt-validation-list--warning">
              <strong>{t("此操作无法撤销", "This action cannot be undone")}</strong>
              <ul>
                <li>
                  {t(
                    "已保存的激活修订不会被修改。",
                    "The saved active revision will not be modified.",
                  )}
                </li>
              </ul>
            </div>
            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button" disabled={busy}>
                  {t("继续编辑", "Keep editing")}
                </button>
              </Dialog.Close>
              <button type="submit" disabled={busy}>
                {busy ? t("刷新中...", "Refreshing...") : t("丢弃并刷新", "Discard and refresh")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
