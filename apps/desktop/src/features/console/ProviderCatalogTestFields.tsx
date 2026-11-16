import { Button, Checkbox, EmptyState, Input, Select, Switch, Textarea } from "@neuro/ui";
import { useState } from "react";
import type { CredentialTestPolicy } from "../../api/contracts";
import { CatalogField, type CatalogFieldsProps } from "./ProviderCatalogFields";

export function ProviderCatalogTestFields({ t, workflow, update, locked }: CatalogFieldsProps) {
  const [editing, setEditing] = useState(false);
  const [selected, setSelected] = useState("connection");
  const policy = workflow.testPolicy;
  const change = (patch: Partial<CredentialTestPolicy>) => update({ testPolicy: { ...policy, ...patch } });
  const active = policy.cases.find((item) => item.id === selected);
  const editCase = (patch: Partial<CredentialTestPolicy["cases"][number]>) => change({ cases: policy.cases.map((item) => item.id === selected ? { ...item, ...patch } : item) });
  if (editing) return <div className="nt-catalog-test-editor">
    <Button onClick={() => setEditing(false)}>{t("← 返回自动测试", "← Back to automatic tests")}</Button>
    <div className="nt-catalog-mapping">
      <section><h3>{t("测试用例", "Test cases")}</h3>
        <div className="nt-catalog-option-list">{policy.cases.map((item) => <div className="nt-catalog-case" key={item.id}>
          <Checkbox aria-label={t(`选用 ${item.name}`, `Use ${item.name}`)} disabled={locked} checked={item.enabled}
            onChange={(event) => change({ cases: policy.cases.map((entry) => entry.id === item.id ? { ...entry, enabled: event.target.checked } : entry) })} />
          <Button variant="quiet" compact aria-pressed={selected === item.id} onClick={() => setSelected(item.id)}>{item.name}</Button>
        </div>)}</div>
        <Button disabled={locked || policy.cases.length >= 16} onClick={() => {
          const id = `temporary-${crypto.randomUUID()}`; change({ cases: [...policy.cases, { id, name: t("新用例", "New case"), prompt: "Reply with only OK.", expectedAnswer: "OK", difficulty: 1, enabled: true }] }); setSelected(id);
        }}>{t("新增用例", "New case")}</Button>
      </section>
      <section>{active ? <div className="nt-catalog-settings-grid">
        <CatalogField name={t("名称", "Name")} wide><Input value={active.name} maxLength={256} disabled={locked} onChange={(event) => editCase({ name: event.target.value })} /></CatalogField>
        <CatalogField name={t("难度", "Difficulty")} wide><Select value={active.difficulty} disabled={locked} onChange={(event) => editCase({ difficulty: Number(event.target.value) })}>{[1, 2, 3].map((value) => <option key={value} value={value}>L{value}</option>)}</Select></CatalogField>
        <CatalogField name={t("提示词", "Prompt")} wide><Textarea rows={5} value={active.prompt} maxLength={8192} disabled={locked} onChange={(event) => editCase({ prompt: event.target.value })} /></CatalogField>
        <CatalogField name={t("标准答案", "Expected answer")} wide><Input value={active.expectedAnswer ?? ""} maxLength={4096} disabled={locked} onChange={(event) => editCase({ expectedAnswer: event.target.value || null })} /></CatalogField>
        <Button variant="danger" disabled={locked} onClick={() => { change({ cases: policy.cases.filter((item) => item.id !== selected) }); setSelected(policy.cases.find((item) => item.id !== selected)?.id ?? ""); }}>{t("删除用例", "Delete case")}</Button>
      </div> : <EmptyState title={t("未选择用例", "No case selected")} />}</section>
    </div>
  </div>;
  return <div className="nt-catalog-settings-grid">
    <CatalogField name={t("自动测试", "Automatic tests")}><Switch disabled={locked} checked={policy.automaticEnabled}
      onChange={(event) => change({ automaticEnabled: event.target.checked })} /></CatalogField>
    <CatalogField name={t("间隔（分钟）", "Interval (minutes)")}><Input type="number" min={1} max={10080} disabled={locked || !policy.automaticEnabled}
      value={policy.intervalMinutes} onChange={(event) => change({ intervalMinutes: Number(event.target.value) })} /></CatalogField>
    <CatalogField name={t("测试范围", "Test scope")}><Input readOnly value={t("整个凭据池", "Entire credential pool")} /></CatalogField>
    <CatalogField name={t("测试模型", "Test models")}><Select disabled={locked} value={policy.modelSelection}
      onChange={(event) => change({ modelSelection: event.target.value as CredentialTestPolicy["modelSelection"] })}><option value="all">{t("全部模型", "All models")}</option><option value="selected">{t("指定模型", "Selected models")}</option></Select></CatalogField>
    <CatalogField name={t("指定模型", "Selected model names")} wide><Input disabled={locked || policy.modelSelection !== "selected"} value={policy.models.join(", ")}
      onChange={(event) => change({ models: event.target.value.split(",").map((value) => value.trim()) })} /></CatalogField>
    <div className="nt-catalog-field--wide"><div className="nt-catalog-inline">
      <span>{t(`已选 ${policy.cases.filter((item) => item.enabled).length} 项`, `${policy.cases.filter((item) => item.enabled).length} selected`)}</span>
      <Button onClick={() => setEditing(true)}>{t("配置测试集", "Configure test set")}</Button>
    </div></div>
  </div>;
}
