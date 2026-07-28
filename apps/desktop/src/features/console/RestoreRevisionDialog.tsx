import * as Dialog from "@radix-ui/react-dialog";
import { type FormEvent } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export type RestoreRevisionDialogProps = {
  open: boolean;
  busy: boolean;
  activeRevisionId: string;
  selectedRevisionId: string;
  commitMessage: string;
  secretPatchSummary: {
    keep: number;
    replace: number;
    clear: number;
  };
  aliasChangeLines: string[];
  providerChangeLines: string[];
  modelRouteChangeLines: string[];
  accountGroupChangeLines: string[];
  onOpenChange(open: boolean): void;
  onConfirm(): Promise<void>;
};

function renderChangeBlock(title: string, lines: string[], emptyMessage: string) {
  return (
    <div className="nt-validation-list nt-validation-list--warning">
      <strong>{title}</strong>
      <ul>
        {lines.length > 0 ? (
          lines.map((line) => <li key={`${title}:${line}`}>{line}</li>)
        ) : (
          <li>{emptyMessage}</li>
        )}
      </ul>
    </div>
  );
}

export function RestoreRevisionDialog({
  open,
  busy,
  activeRevisionId,
  selectedRevisionId,
  commitMessage,
  secretPatchSummary,
  aliasChangeLines,
  providerChangeLines,
  modelRouteChangeLines,
  accountGroupChangeLines,
  onOpenChange,
  onConfirm,
}: RestoreRevisionDialogProps) {
  const { t } = useUiLocale();

  const submit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    try {
      await onConfirm();
      onOpenChange(false);
    } catch {
      // The caller surfaces the server-safe error message in the main console area.
    }
  };

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content" aria-describedby="restore-revision-description">
          <Dialog.Title>{t("确认恢复修订", "Review revision restore")}</Dialog.Title>
          <Dialog.Description id="restore-revision-description">
            {t(
              "确认使用选中的历史修订替换当前激活的路由配置。",
              "Confirm that the selected archived revision should replace the current active route configuration.",
            )}
          </Dialog.Description>
          <form onSubmit={(event) => void submit(event)} className="nt-stack">
            <dl className="nt-meta-list">
              <div>
                <dt>{t("当前激活修订", "Current active revision")}</dt>
                <dd>{activeRevisionId}</dd>
              </div>
              <div>
                <dt>{t("准备恢复的修订", "Revision to restore")}</dt>
                <dd>{selectedRevisionId}</dd>
              </div>
              <div>
                <dt>{t("提交说明", "Commit message")}</dt>
                <dd>{commitMessage}</dd>
              </div>
            </dl>

            <div className="nt-validation-list nt-validation-list--warning">
              <strong>{t("敏感字段补丁计划", "Secret patch plan")}</strong>
              <ul>
                <li>{t("保留", "keep")}: {secretPatchSummary.keep}</li>
                <li>{t("替换", "replace")}: {secretPatchSummary.replace}</li>
                <li>{t("清空", "clear")}: {secretPatchSummary.clear}</li>
              </ul>
            </div>

            <div className="nt-stack">
              <div className="nt-validation-list nt-validation-list--warning">
                <strong>{t("恢复差异复核", "Restore diff review")}</strong>
                <ul>
                  <li>
                    {t(
                      "在提升历史修订为激活配置之前，请逐项检查下面的变更块。",
                      "Review each change block before promoting the archived revision.",
                    )}
                  </li>
                </ul>
              </div>
              <div className="nt-grid nt-grid--2">
                {renderChangeBlock(
                  t("Alias 变更", "Alias changes"),
                  aliasChangeLines,
                  t("未检测到 Alias 变更。", "No alias changes detected."),
                )}
                {renderChangeBlock(
                  t("Provider 变更", "Provider changes"),
                  providerChangeLines,
                  t("未检测到 Provider 变更。", "No provider changes detected."),
                )}
                {renderChangeBlock(
                  t("模型路由变更", "Model route changes"),
                  modelRouteChangeLines,
                  t("未检测到模型路由变更。", "No model route changes detected."),
                )}
                {renderChangeBlock(
                  t("账号分组变更", "Account group changes"),
                  accountGroupChangeLines,
                  t("未检测到账号分组变更。", "No account group changes detected."),
                )}
              </div>
            </div>

            <div className="dialog-actions">
              <Dialog.Close asChild>
                <button type="button" disabled={busy}>
                  {t("取消", "Cancel")}
                </button>
              </Dialog.Close>
              <button type="submit" disabled={busy}>
                {busy ? t("恢复中...", "Restoring...") : t("确认恢复", "Confirm restore")}
              </button>
            </div>
          </form>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
