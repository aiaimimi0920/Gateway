import { useState } from "react";
import type { CredentialTestPolicy } from "../../api/contracts";
import type { TranslateFn } from "./accountsLedgerTypes";
import { appendIqCases } from "./credentialIqBank";

export function CredentialTestPlanFields({ policy, models, configuredModels, locked, onChange, t }: {
  policy: CredentialTestPolicy; models: string[]; configuredModels: string[]; locked: boolean; onChange(policy: CredentialTestPolicy): void; t: TranslateFn;
}) {
  const [model, setModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [expected, setExpected] = useState("");
  const [difficulty, setDifficulty] = useState(1);
  const [temporaryError, setTemporaryError] = useState<string | null>(null);
  const selectModel = (id: string, selected: boolean) => onChange({ ...policy, modelSelection: "selected", models: selected
    ? [...new Set([...policy.models, id])] : policy.models.filter((entry) => entry !== id) });
  return <>
    <fieldset className="nt-test-selection" disabled={locked}>
      <legend>{t("测试模型", "Test models")}</legend>
      <div className="nt-test-selection__toolbar">
        <label><input type="checkbox" checked={policy.modelSelection === "all"} onChange={(event) => onChange({ ...policy,
          modelSelection: event.target.checked ? "all" : "selected", models: policy.models.length ? policy.models : configuredModels })} />{t("全部模型", "All models")}</label>
        <button type="button" className="nt-btn nt-btn--outline" onClick={() => onChange({ ...policy, modelSelection: "selected", models: [] })}>{t("清空选择", "Clear selection")}</button>
      </div>
      <div className="nt-test-selection__list">
        {models.map((id) => <label key={id}><input type="checkbox" checked={policy.modelSelection === "all" ? configuredModels.includes(id) : policy.models.includes(id)}
          onChange={(event) => {
            const selected = policy.modelSelection === "all" ? configuredModels : policy.models;
            onChange({ ...policy, modelSelection: "selected", models: event.target.checked ? [...new Set([...selected, id])] : selected.filter((entry) => entry !== id) });
          }} /><span>{id}</span></label>)}
        {models.length === 0 ? <p className="nt-copy">{t("暂无模型", "No models")}</p> : null}
      </div>
      <div className="nt-test-inline">
        <input className="nt-input" aria-label={t("添加测试模型", "Add test model")} maxLength={256} value={model} onChange={(event) => setModel(event.target.value)} placeholder={t("精确模型名", "Exact model name")} />
        <button type="button" className="nt-btn nt-btn--outline" disabled={!model.trim()} onClick={() => { selectModel(model.trim(), true); setModel(""); }}>{t("添加模型", "Add model")}</button>
      </div>
    </fieldset>
    <fieldset className="nt-test-selection" disabled={locked}>
      <legend>{t(`测试集 (${policy.cases.filter((item) => item.enabled).length}/${policy.cases.length})`, `Test set (${policy.cases.filter((item) => item.enabled).length}/${policy.cases.length})`)}</legend>
      <div className="nt-test-selection__toolbar">
        <button type="button" className="nt-btn nt-btn--outline" onClick={() => onChange({ ...policy, cases: policy.cases.map((item) => ({ ...item, enabled: true })) })}>{t("全选测试集", "Select all cases")}</button>
        <button type="button" className="nt-btn nt-btn--outline" onClick={() => onChange({ ...policy, cases: policy.cases.map((item) => ({ ...item, enabled: false })) })}>{t("清空测试集", "Clear cases")}</button>
        <button type="button" className="nt-btn nt-btn--outline" disabled={appendIqCases(policy.cases).length === policy.cases.length}
          onClick={() => onChange({ ...policy, cases: appendIqCases(policy.cases) })}>{t("补充 IQ 题库", "Add IQ questions")}</button>
      </div>
      <div className="nt-test-selection__list nt-test-cases">
        {policy.cases.map((item) => <article key={item.id}>
          <label><input type="checkbox" checked={item.enabled} onChange={(event) => onChange({ ...policy, cases: policy.cases.map((entry) => entry.id === item.id ? { ...entry, enabled: event.target.checked } : entry) })} />
            <span>{item.name}</span><small>L{item.difficulty} · {item.expectedAnswer ? t("可评分", "Scored") : t("仅联通", "Connectivity only")}</small></label>
          <details><summary>{t("用例详情", "Case details")}</summary><p>{item.prompt}</p><p>{t("标准答案：", "Expected: ")}{item.expectedAnswer ?? "—"}</p></details>
          {item.id.startsWith("temporary-") ? <button type="button" className="nt-btn nt-btn--outline" onClick={() => onChange({ ...policy, cases: policy.cases.filter((entry) => entry.id !== item.id) })}>{t("移除临时用例", "Remove temporary case")}</button> : null}
        </article>)}
      </div>
    </fieldset>
    <fieldset className="nt-test-temporary" disabled={locked}>
      <legend>{t("临时用例", "Temporary case")}</legend>
      <textarea className="nt-input" rows={3} aria-label={t("临时提示词", "Temporary prompt")} maxLength={8192} value={prompt} onChange={(event) => setPrompt(event.target.value)} />
      <div className="nt-test-inline">
        <input className="nt-input" aria-label={t("标准答案（可选）", "Expected answer (optional)")} maxLength={4096} value={expected} onChange={(event) => setExpected(event.target.value)} placeholder={t("标准答案（可选）", "Expected answer (optional)")} />
        <label className="nt-field"><span>{t("用例难度", "Difficulty")}</span><select className="nt-input" value={difficulty} onChange={(event) => setDifficulty(Number(event.target.value))}><option value={1}>L1</option><option value={2}>L2</option><option value={3}>L3</option></select></label>
      </div>
      {temporaryError ? <p role="alert" className="nt-copy">{temporaryError}</p> : null}
      <button type="button" className="nt-btn nt-btn--outline" disabled={!prompt.trim() || policy.cases.length >= 16} onClick={() => {
        if (new TextEncoder().encode(prompt).length > 8192 || new TextEncoder().encode(expected).length > 4096) { setTemporaryError(t("提示词或标准答案超过字节限制。", "Prompt or expected answer exceeds the byte limit.")); return; }
        onChange({ ...policy, cases: [...policy.cases, { id: `temporary-${crypto.randomUUID()}`, name: prompt.trim().slice(0, 32), prompt, expectedAnswer: expected.trim() || null, difficulty, enabled: true }] });
        setPrompt(""); setExpected(""); setTemporaryError(null);
      }}>{t("加入测试集", "Add to test set")}</button>
    </fieldset>
  </>;
}
