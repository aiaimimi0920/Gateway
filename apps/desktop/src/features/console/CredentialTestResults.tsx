import type { ConsoleProviderProbeResponse } from "../../api/contracts";
import type { TranslateFn } from "./accountsLedgerTypes";
import type { TestSelectionAccount } from "./credentialTestSelection";
import { testScopeKey } from "./credentialTestPolicyDocument";
import { testCapabilityLevels as LEVELS } from "./credentialTestSummary";

export function CredentialTestResults({ response, responses, accounts, busy, error, t, planId }: {
  response: ConsoleProviderProbeResponse | null; responses: ConsoleProviderProbeResponse[]; accounts: TestSelectionAccount[]; busy: boolean; error: string | null; t: TranslateFn;
  planId?: string | null;
}) {
  const names = new Map(accounts.map((account) => [account.key, account.name]));
  const entries = (responses.length ? responses : response ? [response] : []).flatMap((entry) => entry.result.results.map((result) => ({
    result, key: testScopeKey({ kind: "account", providerId: entry.result.providerId, id: result.credentialId }) })));
  // Undefined permits the manager's already-filtered collection; null selects legacy results only.
  const results = [...new Map(entries.filter((entry) => names.has(entry.key)
    && (planId === undefined || (entry.result.assessment?.planId ?? null) === planId))
    .map((entry) => [JSON.stringify([entry.key, entry.result.assessment?.planId]), entry])).values()];
  return <section className="nt-test-results" aria-label={t("测试结果", "Test results")} aria-live="polite">
    <h3>{t("测试结果", "Test results")}</h3>
    {busy ? <p role="status">{t("测试中…", "Testing…")}</p> : null}
    {error ? <p role="alert" className="nt-banner nt-banner--danger">{error}</p> : null}
    {!results.length && !busy ? <p className="nt-copy">{t("暂无结果", "No results")}</p> : null}
    {results.map(({ result, key }) => <article className="nt-test-result" key={JSON.stringify([key, result.assessment?.planId])}>
      <h4>{names.get(key)}</h4><small>{result.checkedAt} · {result.assessment?.mode === "automatic" ? t("自动", "Auto") : t("手动", "Manual")}</small>
      {result.status !== "passed" ? <p>{result.message}</p> : null}
      {result.assessment ? <>
        {result.assessment.models.map((model) => {
          const level = LEVELS[model.capabilityLevel] ?? LEVELS.unrated;
          return <div className="nt-test-model-result" key={model.model}><strong>{model.model}</strong>
            <span>{model.callable ? t("可联通调用", "Callable") : t("未证明可调用", "Not proven callable")}</span>
            <span>{t(...level)} · {model.score == null ? "—" : `${model.score}%`} ({model.correctCount}/{model.gradedCount})</span>
          </div>;
        })}
        <details><summary>{t("答案详情", "Answers")}</summary>
          {result.assessment.cases.map((item) => <div className="nt-test-case-result" key={`${item.model}:${item.caseId}`}>
            <strong>{item.model} · {item.name}</strong>
            <p>{t("调用：", "Call: ")}{item.status} · {item.correct === null ? t("未评分", "Not graded") : item.correct ? t("答案正确", "Correct answer") : t("答案不符合标准", "Answer differs from expected")}</p>
            <p>{t("实际回复：", "Answer: ")}{item.answer ?? "—"}</p><p>{t("标准答案：", "Expected: ")}{item.expectedAnswer ?? "—"}</p>
            {item.status !== "passed" ? <p>{item.message}</p> : null}
          </div>)}
        </details>
      </> : <p>{result.probePoint}</p>}
    </article>)}
  </section>;
}
