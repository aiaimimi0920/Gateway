import { Notice } from "@neuro/ui";

type ConsoleRouteFeedbackOptions = {
  activeRouteDiagnostics: readonly { code: string; path: string; message: string }[];
  accountGroupSummaryError: string | null;
  draftDocumentState: { invalid: boolean };
  t: (zh: string, en: string) => string;
};

export function buildConsoleRouteFeedback({
  activeRouteDiagnostics,
  accountGroupSummaryError,
  draftDocumentState,
  t,
}: ConsoleRouteFeedbackOptions) {
  const activeDiagnosticsFeedback = activeRouteDiagnostics.length > 0 ? (
    <Notice
      tone="danger"
      className="nt-diagnostics-report"
      aria-label={t("Active route diagnostics", "Active route diagnostics")}
    >
      <div>
        <strong>
          {t(
            `当前激活路由有 ${activeRouteDiagnostics.length} 条需要处理的诊断`,
            `${activeRouteDiagnostics.length} active route diagnostic(s) require attention`,
          )}
        </strong>
        <ul>
          {activeRouteDiagnostics.map((diagnostic) => (
            <li key={`${diagnostic.code}:${diagnostic.path}:${diagnostic.message}`}>
              <code>{diagnostic.code}</code> · <code>{diagnostic.path}</code> · {diagnostic.message}
            </li>
          ))}
        </ul>
        <p className="nt-copy">
          {t(
            "请联系管理员处理这些路由诊断问题，再保存新的路由修订。",
            "Contact an administrator to resolve these route diagnostics before saving a new revision.",
          )}
        </p>
      </div>
    </Notice>
  ) : null;
  const accountSummaryFeedback = accountGroupSummaryError ? (
    <Notice
      tone="warning"
      aria-label={t("Account summary status", "Account summary status")}
    >
      <span>
        {t(
          `后端账号汇总暂不可用，当前显示本地可解析快照：${accountGroupSummaryError}`,
          `Backend account summary unavailable; showing the locally parseable snapshot: ${accountGroupSummaryError}`,
        )}
      </span>
    </Notice>
  ) : null;
  const draftStructureNotice = draftDocumentState.invalid ? (
    <Notice tone="warning" className="nt-diagnostics-report">
      <strong>{t("当前 JSON 草稿不可解析", "Current JSON draft is invalid")}</strong>
      <ul>
        <li>
          {t(
            "账号与分组视图当前回退到上一份可解析快照；请联系管理员修复路由草稿。",
            "Accounts and groups currently fall back to the last parseable snapshot. Contact an administrator to repair the route draft.",
          )}
        </li>
      </ul>
    </Notice>
  ) : null;
  return { activeDiagnosticsFeedback, accountSummaryFeedback, draftStructureNotice };
}
