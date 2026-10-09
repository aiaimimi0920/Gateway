import type { ConsoleCredentialProbeResult } from "../../api/contracts";
import type { TranslateFn } from "./accountsLedgerTypes";
import { testIqRows } from "./credentialIqSummary";

export function CredentialTestModelSummary({ results, t }: { results: ConsoleCredentialProbeResult[]; t: TranslateFn }) {
  const rows = testIqRows(results);
  const number = (value: number | null) => value === null ? "—" : new Intl.NumberFormat(undefined, { maximumFractionDigits: 4 }).format(value);
  const duration = (ms: number | null) => ms === null ? "—" : ms < 1000 ? `${ms} ms` : `${number(ms / 1000)} s`;
  return <section className="nt-iq-board" aria-label={t("模型 IQ 看板", "Model IQ dashboard")}>
    <p className="nt-copy">{t("IQ 为选定题集的难度加权测试指数（0–100），不是人类 IQ。同题集、同账户、同额度单位才可比较。", "IQ is a difficulty-weighted test index (0–100), not human IQ. Compare the same test set, account and quota unit.")}</p>
    <p className="nt-copy">{t("额度为供应商账户前后观测差额，不是请求级账单；并发使用、结算延迟、精度和重置可能影响归因。观测比值仅供参考。", "Quota is the provider account's observed delta, not request-level billing; concurrency, settlement delay, precision and resets can affect attribution. Ratios are observational only.")}</p>
    {!rows.length ? <p className="nt-copy">{t("暂无模型测试结果", "No model test results")}</p> : null}
    <div className="nt-iq-grid">{rows.map((row) => <article key={row.key} className="nt-iq-card" aria-label={`${row.model} · ${row.account}`}>
      <h3>{row.model}</h3><p className="nt-copy">{row.provider} / {row.account}</p>
      <div className="nt-iq-score"><strong>{number(row.iq)}</strong><span>{t("测试 IQ", "Test IQ")}</span><small>{row.correct}/{row.scored} {t("题正确", "correct")}</small></div>
      <dl className="nt-iq-metrics">
        <div><dt>{t("额度消耗（账户观测）", "Quota consumed (account observation)")}</dt><dd>{number(row.observed)} <small>{row.quota?.unit ?? ""}</small></dd></div>
        <div><dt>{t("完成用时", "Completion time")}</dt><dd>{duration(row.elapsedMs)}</dd></div>
        <div><dt>{t("额度 / IQ · 越低越好", "Quota / IQ · lower is better")}</dt><dd>{number(row.quotaPerIq)}<small>{t("观测比值", "Observed ratio")}</small></dd></div>
        <div><dt>{t("IQ / 额度 · 越高越好", "IQ / quota · higher is better")}</dt><dd>{number(row.iqPerQuota)}<small>{t("观测比值", "Observed ratio")}</small></dd></div>
      </dl>
      <p className="nt-copy">{!row.testSetId ? t("旧结果未记录用时/额度，请重新测试。", "Older result has no measurements. Run a new test.")
        : !row.complete ? t("测试未完整完成或缺少可评分答案，暂不计算 IQ。", "Test is incomplete or ungraded; IQ is unavailable.")
        : row.observed !== null ? t("仅观测到账户变化，无法确认全部由本测试扣除。", "Account change observed; exclusive attribution to this test is unverified.")
        : row.quota?.status === "no-visible-change" ? t("额度无可见变化，不代表免费；比值不可计算。", "No visible quota change does not mean free; ratios unavailable.")
        : row.quota?.status === "reset-or-replenished" ? t("额度发生重置或补充，差额不可计算。", "Quota reset or replenishment; delta unavailable.")
        : t("供应商未提供可核实的额度数据，未使用 token 或估算费用替代。", "Verifiable provider quota is unavailable; tokens or estimated prices are not substituted.")}</p>
      <details><summary>{t("测量明细", "Measurement details")}</summary>
        <p>{t("计划：", "Plan: ")}{row.plan} · {row.attempted}/{row.total} {t("题已执行", "attempted")}</p>
        <p className="nt-iq-fingerprint">{t("题集：", "Test set: ")}{row.testSetId ?? "—"}</p>
        <p>{t("额度前 / 后：", "Quota before / after: ")}{number(row.quota?.before ?? null)} / {number(row.quota?.after ?? null)} {row.quota?.unit}</p>
        <p>{t("来源：", "Source: ")}{row.quota?.source ?? "—"}</p><time>{row.latest}</time>
      </details>
    </article>)}</div>
  </section>;
}
