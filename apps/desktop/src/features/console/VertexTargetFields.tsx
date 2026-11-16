import { Field, Input } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { VertexTarget } from "./vertexOAuth";

export function VertexTargetFields({ value, disabled, fixed = false, quotaFixed = fixed, quota = true, onChange }: {
  value: VertexTarget; disabled: boolean; fixed?: boolean; quotaFixed?: boolean; quota?: boolean; onChange(value: VertexTarget): void;
}) {
  const { t } = useUiLocale();
  return <>
    <Field label="Project"><Input value={value.project} maxLength={256} disabled={disabled} readOnly={fixed}
      autoComplete="off" spellCheck={false} onChange={(event) => { if (!fixed) onChange({ ...value, project: event.target.value }); }} /></Field>
    <Field label="Location"><Input value={value.location} maxLength={52} disabled={disabled} readOnly={fixed}
      autoComplete="off" spellCheck={false} onChange={(event) => { if (!fixed) onChange({ ...value, location: event.target.value }); }} /></Field>
    {quota && <Field label={t("Quota Project（可选）", "Quota project (optional)")}><Input value={value.quota_project ?? ""}
      maxLength={256} disabled={disabled} readOnly={quotaFixed} autoComplete="off" spellCheck={false}
      onChange={(event) => { if (quotaFixed) return; const { quota_project: _previous, ...base } = value;
        onChange(event.target.value ? { ...base, quota_project: event.target.value } : base); }} /></Field>}
  </>;
}
