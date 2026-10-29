import { useState } from "react";
import { CatalogField, type CatalogFieldsProps } from "./ProviderCatalogFields";

export function ProviderCatalogMappingFields({ t, workflow, update, locked, models }: CatalogFieldsProps & { models: string[] }) {
  const [source, setSource] = useState("");
  const [target, setTarget] = useState("");
  const [selected, setSelected] = useState("");
  const [error, setError] = useState("");
  const mappings = workflow.mappings;
  // Model IDs are data, including names such as __proto__ and constructor.
  const mappedTargets = (name: string) => Object.hasOwn(mappings, name) ? mappings[name] : [];
  const sources = [...new Set([...models, ...Object.keys(mappings)])];
  const targets = [...new Set([...models, ...Object.values(mappings).flat()])];
  const valid = (value: string) => value.trim() === value && value.length > 0 && new TextEncoder().encode(value).length <= 256 && !/[*?\u0000-\u001f\u007f-\u009f]/u.test(value);
  const associate = (value: string, checked: boolean) => {
    const values = mappedTargets(selected);
    if (!selected) return;
    if (checked && values.length >= 32) { setError(t("每个模型最多关联 32 个目标。", "At most 32 targets per model.")); return; }
    update({ mappings: { ...mappings, [selected]: checked ? [...new Set([...values, value])] : values.filter((entry) => entry !== value) } });
    setError("");
  };
  return <div className="nt-catalog-mapping">
    <section><h3>{t("用户模型", "Requested model")}</h3>
      <div className="nt-catalog-option-list">{sources.map((name) => <button type="button" key={name} aria-pressed={selected === name}
        onClick={() => setSelected(name)}>{name}</button>)}</div>
      <CatalogField name={t("自定义模型", "Custom model")}><input className="nt-input" value={source} maxLength={256} disabled={locked} onChange={(event) => setSource(event.target.value)} /></CatalogField>
      <button type="button" className="nt-btn nt-btn--outline" disabled={locked || !source.trim()} onClick={() => {
        if (!valid(source) || sources.length >= 512) { setError(t("请输入精确模型名，最多 512 个映射。", "Enter an exact model name, at most 512 mappings.")); return; }
        update({ mappings: { ...mappings, [source]: mappedTargets(source) } }); setSelected(source); setSource(""); setError("");
      }}>{t("加入", "Add")}</button>
    </section>
    <section><h3>{t("上游模型", "Upstream model")}</h3>
      <div className="nt-catalog-option-list">{targets.map((name) => <label key={name}><input type="checkbox" disabled={locked || !selected}
        checked={mappedTargets(selected).includes(name)} onChange={(event) => associate(name, event.target.checked)} />{name}</label>)}</div>
      <CatalogField name={t("自定义目标", "Custom target")}><input className="nt-input" value={target} maxLength={256} disabled={locked || !selected} onChange={(event) => setTarget(event.target.value)} /></CatalogField>
      <button type="button" className="nt-btn nt-btn--outline" disabled={locked || !selected || !target.trim()} onClick={() => {
        if (!valid(target)) { setError(t("请输入精确模型名。", "Enter an exact model name.")); return; } associate(target, true); setTarget("");
      }}>{t("关联", "Link")}</button>
      <button type="button" className="nt-btn nt-btn--outline" disabled={locked || !mappedTargets(selected).length} onClick={() => update({ mappings: { ...mappings, [selected]: [] } })}>{t("清空关联", "Clear links")}</button>
    </section>
    {error && <p role="alert">{error}</p>}
  </div>;
}
