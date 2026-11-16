import { useState } from "react";
import { Button, Checkbox, CheckboxGroup, Disclosure, Field, Input, MultiSelect, Notice, Select, Textarea } from "@neuro/ui";
import type { CredentialTestPolicy } from "../../api/contracts";
import type { TranslateFn } from "./accountsLedgerTypes";
import { appendIqCases } from "./credentialIqBank";
import { modelIdLimit, validModelId, type ModelIdPolicy } from "./providerModelId";

export function CredentialTestPlanFields({ policy, models, configuredModels, locked, onChange, t, modelPolicy = "standard" }: {
  policy: CredentialTestPolicy; models: string[]; configuredModels: string[]; locked: boolean; onChange(policy: CredentialTestPolicy): void; t: TranslateFn;
  modelPolicy?: ModelIdPolicy;
}) {
  const [model, setModel] = useState("");
  const [prompt, setPrompt] = useState("");
  const [expected, setExpected] = useState("");
  const [difficulty, setDifficulty] = useState(1);
  const [temporaryError, setTemporaryError] = useState<string | null>(null);
  // Keep custom candidates mounted when their last selection is removed.
  const [sessionModels, setSessionModels] = useState(models);
  const candidates = [...new Set([...sessionModels, ...models])];
  const selectedModels = policy.modelSelection === "all" ? configuredModels : policy.models;
  const rememberModels = () => setSessionModels(candidates);
  const changeModels = (values: string[]) => { rememberModels(); onChange({ ...policy, modelSelection: "selected", models: values }); };
  const selectModel = (id: string, selected: boolean) => onChange({
    ...policy, modelSelection: "selected", models: selected
      ? [...new Set([...policy.models, id])] : policy.models.filter((entry) => entry !== id)
  });
  return <>
    <fieldset className="nt-test-selection n-choice-group" disabled={locked}>
      <legend>{t("测试模型", "Test models")}</legend>
      <div className="nt-test-selection__toolbar">
        <Checkbox label={t("全部模型", "All models")} checked={policy.modelSelection === "all"} onChange={(event) => {
          rememberModels(); onChange({
            ...policy,
            modelSelection: event.target.checked ? "all" : "selected", models: policy.models.length ? policy.models : configuredModels
          });
        }} />
        <Button compact onClick={() => changeModels([])}>{t("清空选择", "Clear selection")}</Button>
      </div>
      <div className="nt-test-selection__list">
        {candidates.length > 6 ? <MultiSelect label={t("选择模型", "Select models")} options={candidates.map((id) => ({ value: id, label: id }))}
          values={selectedModels} onValuesChange={changeModels} disabled={locked} compact formatSummary={(count) => t(`已选 ${count} 项`, `${count} selected`)}
          search={{ label: t("搜索模型", "Search models"), emptyMessage: t("无匹配项", "No matches") }} />
          : <CheckboxGroup label={t("选择模型", "Select models")} options={candidates.map((id) => ({ value: id, label: id }))}
            values={selectedModels} onValuesChange={changeModels} disabled={locked} />}
        {candidates.length === 0 ? <span className="n-muted">{t("暂无模型", "No models")}</span> : null}
      </div>
      <div className="nt-test-inline">
        <Input compact aria-label={t("添加测试模型", "Add test model")} maxLength={modelIdLimit(modelPolicy)} value={model} onChange={(event) => setModel(event.target.value)} placeholder={t("精确模型名", "Exact model name")} />
        <Button compact disabled={!validModelId(model.trim(), modelPolicy)} onClick={() => { setSessionModels([...new Set([...candidates, model.trim()])]); selectModel(model.trim(), true); setModel(""); }}>{t("添加模型", "Add model")}</Button>
      </div>
    </fieldset>
    <fieldset className="nt-test-selection n-choice-group" disabled={locked}>
      <legend>{t(`测试集 (${policy.cases.filter((item) => item.enabled).length}/${policy.cases.length})`, `Test set (${policy.cases.filter((item) => item.enabled).length}/${policy.cases.length})`)}</legend>
      <div className="nt-test-selection__toolbar">
        <Button compact wrap onClick={() => onChange({ ...policy, cases: policy.cases.map((item) => ({ ...item, enabled: true })) })}>{t("全选测试集", "Select all cases")}</Button>
        <Button compact wrap onClick={() => onChange({ ...policy, cases: policy.cases.map((item) => ({ ...item, enabled: false })) })}>{t("清空测试集", "Clear cases")}</Button>
        <Button compact wrap disabled={appendIqCases(policy.cases).length === policy.cases.length}
          onClick={() => onChange({ ...policy, cases: appendIqCases(policy.cases) })}>{t("补充 IQ 题库", "Add IQ questions")}</Button>
      </div>
      <div className="nt-test-selection__list nt-test-cases">
        {policy.cases.map((item) => <article key={item.id}>
          <div className="nt-test-case-heading"><Checkbox label={item.name} checked={item.enabled} onChange={(event) => onChange({ ...policy, cases: policy.cases.map((entry) => entry.id === item.id ? { ...entry, enabled: event.target.checked } : entry) })} />
            <small>L{item.difficulty} · {item.expectedAnswer ? t("可评分", "Scored") : t("仅联通", "Connectivity only")}</small></div>
          <Disclosure title={t("用例详情", "Case details")}><p>{item.prompt}</p><p>{t("标准答案：", "Expected: ")}{item.expectedAnswer ?? "—"}</p></Disclosure>
          {item.id.startsWith("temporary-") ? <Button compact wrap onClick={() => onChange({ ...policy, cases: policy.cases.filter((entry) => entry.id !== item.id) })}>{t("移除临时用例", "Remove temporary case")}</Button> : null}
        </article>)}
      </div>
    </fieldset>
    <fieldset className="nt-test-temporary n-choice-group" disabled={locked}>
      <legend>{t("临时用例", "Temporary case")}</legend>
      <Textarea compact rows={3} aria-label={t("临时提示词", "Temporary prompt")} maxLength={8192} value={prompt} onChange={(event) => setPrompt(event.target.value)} />
      <div className="nt-test-inline">
        <Field label={t("标准答案（可选）", "Expected answer (optional)")}><Input compact maxLength={4096} value={expected} onChange={(event) => setExpected(event.target.value)} /></Field>
        <Field label={t("用例难度", "Difficulty")}><Select compact value={difficulty} onChange={(event) => setDifficulty(Number(event.target.value))}><option value={1}>L1</option><option value={2}>L2</option><option value={3}>L3</option></Select></Field>
      </div>
      {temporaryError ? <Notice tone="danger">{temporaryError}</Notice> : null}
      <Button compact wrap disabled={!prompt.trim() || policy.cases.length >= 16} onClick={() => {
        if (new TextEncoder().encode(prompt).length > 8192 || new TextEncoder().encode(expected).length > 4096) { setTemporaryError(t("提示词或标准答案超过字节限制。", "Prompt or expected answer exceeds the byte limit.")); return; }
        onChange({ ...policy, cases: [...policy.cases, { id: `temporary-${crypto.randomUUID()}`, name: prompt.trim().slice(0, 32), prompt, expectedAnswer: expected.trim() || null, difficulty, enabled: true }] });
        setPrompt(""); setExpected(""); setTemporaryError(null);
      }}>{t("加入测试集", "Add to test set")}</Button>
    </fieldset>
  </>;
}
