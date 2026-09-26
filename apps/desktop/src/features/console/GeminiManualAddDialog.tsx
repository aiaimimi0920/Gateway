import { X } from "lucide-react";
import * as Dialog from "@radix-ui/react-dialog";
import { useRef } from "react";
import { canManuallyCompleteGeminiAuthSession } from "./geminiCredentialDraft";
import type { useGeminiManualAddSession } from "./useGeminiManualAddSession";

type ManualAddSession = ReturnType<typeof useGeminiManualAddSession>;
type GeminiManualAddDialogProps = {
  geminiManualAddDialogState: ManualAddSession["geminiManualAddDialogState"];
  closeGeminiManualAddDialog: ManualAddSession["closeGeminiManualAddDialog"];
  requestGeminiManualAddCompletion: ManualAddSession["requestGeminiManualAddCompletion"];
  t: (zh: string, en: string) => string;
};

export function GeminiManualAddDialog({
  geminiManualAddDialogState,
  closeGeminiManualAddDialog,
  requestGeminiManualAddCompletion,
  t,
}: GeminiManualAddDialogProps) {
  const openerRef = useRef<HTMLElement | null>(null);
  if (!geminiManualAddDialogState) {
    return null;
  }
  return (
    <Dialog.Root open onOpenChange={(open) => {
      if (!open) closeGeminiManualAddDialog();
    }}>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className="dialog-content nt-pilot-dialog"
          aria-describedby={undefined}
          onOpenAutoFocus={() => {
            openerRef.current = document.activeElement instanceof HTMLElement
              ? document.activeElement : null;
          }}
          onCloseAutoFocus={(event) => {
            // This controlled dialog has no Radix trigger to restore focus to.
            event.preventDefault();
            if (openerRef.current?.isConnected) openerRef.current.focus();
          }}
        >
          <div className="nt-pilot-dialog__header">
            <Dialog.Title asChild>
              <h2>{t("Gemini 手动添加", "Gemini manual add")}</h2>
            </Dialog.Title>
            <button
              className="nt-icon-close"
              type="button"
              aria-label={t("关闭", "Close")}
              onClick={closeGeminiManualAddDialog}
            >
              <X size={18} aria-hidden="true" />
            </button>
          </div>
          <div className="nt-stack">
            <article className="nt-card nt-card--panel">
              <div className="nt-copy">
                <strong>{geminiManualAddDialogState.providerId}</strong>
              </div>
              <p className="nt-copy" role="status" aria-live="polite" aria-atomic="true">
                {geminiManualAddDialogState.busy
                  ? t("正在启动本地 Gemini 认证助手。", "Starting the local Gemini auth helper.")
                  : geminiManualAddDialogState.session?.message ??
                    t("等待 Gemini 认证结果。", "Waiting for Gemini auth results.")}
              </p>
              {geminiManualAddDialogState.session ? (
                <div className="nt-ledger-chip-list nt-ledger-chip-list--dense">
                  <span className="nt-chip nt-chip--muted">
                    {t("状态", "Status")} {geminiManualAddDialogState.session.status}
                  </span>
                  <span className="nt-chip nt-chip--muted">
                    {t("目标族", "Target family")} {geminiManualAddDialogState.session.targetFamily}
                  </span>
                </div>
              ) : null}
            </article>
            {geminiManualAddDialogState.session?.status === "succeeded" ? (
              <article className="nt-card nt-card--panel">
                <p className="nt-copy">
                  {t(
                    "认证结果已经写入当前草稿。保存路由配置后生效。",
                    "The generated credentials were merged into the current draft. Save the route config to apply them.",
                  )}
                </p>
              </article>
            ) : null}
            {canManuallyCompleteGeminiAuthSession(geminiManualAddDialogState.session) ? (
              <article className="nt-card nt-card--panel">
                <p className="nt-copy">
                  {t(
                    "如果你已经在弹出的浏览器里完成 Gemini 登录，但仍然没有自动导入，请点击下面的按钮继续导入。",
                    "If you already completed Gemini login in the opened browser window but the import did not finish automatically, click the button below to continue the import.",
                  )}
                </p>
                <div className="dialog-actions">
                  <button
                    className="nt-btn"
                    type="button"
                    disabled={geminiManualAddDialogState.busy}
                    onClick={() => {
                      void requestGeminiManualAddCompletion();
                    }}
                  >
                    {geminiManualAddDialogState.busy
                      ? t("正在导入…", "Importing…")
                      : t("已完成登录，继续导入", "I finished login, continue import")}
                  </button>
                </div>
              </article>
            ) : null}
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
