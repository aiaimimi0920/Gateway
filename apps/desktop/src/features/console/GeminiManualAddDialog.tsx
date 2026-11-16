import { Badge, Button, Dialog, Notice, Stack, Toolbar } from "@neuro/ui";
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
  if (!geminiManualAddDialogState) {
    return null;
  }
  const aistudio = geminiManualAddDialogState.targetFamily === "aistudio-web";
  const flowmusic = geminiManualAddDialogState.targetFamily === "flowmusic-web";
  const flow = geminiManualAddDialogState.targetFamily === "flow-web";
  const stitch = geminiManualAddDialogState.targetFamily === "stitch-web";
  return (
    <Dialog open closeLabel={t("关闭", "Close")} onOpenChange={(open) => {
      if (!open) closeGeminiManualAddDialog();
    }} title={stitch ? t("Google Stitch 账号导入", "Google Stitch account import") : flow ? t("Google Flow 账号导入", "Google Flow account import") : flowmusic ? t("FlowMusic 账号导入", "FlowMusic account import") : aistudio ? t("AI Studio 账号导入", "AI Studio account import") : t("Gemini 手动添加", "Gemini manual add")}>
      {/* Closing remains available while the application owns a pending import. */}
      <Stack>
        <strong>{geminiManualAddDialogState.providerId}</strong>
        <Notice aria-live="polite" aria-atomic="true">
          {geminiManualAddDialogState.busy
            ? geminiManualAddDialogState.importMethod === "json"
              ? t("正在解析网页会话 JSON。", "Parsing web session JSON.")
              : t("正在启动网页登录助手。", "Starting the web sign-in helper.")
            : geminiManualAddDialogState.session?.message ??
            t("等待网页登录认证结果。", "Waiting for web sign-in results.")}
        </Notice>
        {geminiManualAddDialogState.session ? (
          <Toolbar>
            <Badge>
              {t("状态", "Status")} {geminiManualAddDialogState.session.status}
            </Badge>
            <Badge>
              {t("目标族", "Target family")} {geminiManualAddDialogState.session.targetFamily}
            </Badge>
          </Toolbar>
        ) : null}
        {geminiManualAddDialogState.session?.status === "succeeded" ? (
          <Notice tone="success" role="note">
            {t(
              "账号材料已经写入当前草稿。保存路由配置后生效。",
              "The generated credentials were merged into the current draft. Save the route config to apply them.",
            )}
          </Notice>
        ) : null}
        {canManuallyCompleteGeminiAuthSession(geminiManualAddDialogState.session) ? (
          <Stack>
            <p className="n-muted">
              {t(
                "如果你已经在弹出的浏览器里完成 Gemini 登录，但仍然没有自动导入，请点击下面的按钮继续导入。",
                "If you already completed Gemini login in the opened browser window but the import did not finish automatically, click the button below to continue the import.",
              )}
            </p>
            <Button
              variant="primary"
              wrap
              loading={geminiManualAddDialogState.busy}
              aria-label={geminiManualAddDialogState.busy ? t("正在导入…", "Importing…") : undefined}
              disabled={geminiManualAddDialogState.busy}
              onClick={() => {
                void requestGeminiManualAddCompletion();
              }}
            >
              {t("已完成登录，继续导入", "I finished login, continue import")}
            </Button>
          </Stack>
        ) : null}
      </Stack>
    </Dialog>
  );
}
