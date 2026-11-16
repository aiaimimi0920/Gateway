import { Field, Input, Notice, Select } from "@neuro/ui";
import { buildQwenEndpoint, QWEN_REGIONS, type QwenEndpointDraft } from "./qwenEndpoint";

export function QwenEndpointFields({ value, onChange, disabled, t }: {
  value: QwenEndpointDraft; onChange(value: QwenEndpointDraft): void; disabled?: boolean;
  t(zh: string, en: string): string;
}) {
  return <fieldset className="nt-qwen-endpoint-fields" disabled={disabled}>
    <legend>{t("Qwen 地域与工作空间", "Qwen region and workspace")}</legend>
    <Field label={t("Qwen 地域", "Qwen region")}>
      <Select value={value.region} onChange={(e) => onChange({ ...value, region: e.target.value })}>
        <option value="">{t("请选择地域", "Select a region")}</option>
        {QWEN_REGIONS.map(([region, zh, en]) => <option key={region} value={region}>{t(zh, en)} ({region})</option>)}
      </Select></Field>
    <Field label={t("Qwen 工作空间", "Qwen workspace")}>
      <Input value={value.workspaceId} maxLength={63} spellCheck={false} autoComplete="off"
        onChange={(e) => onChange({ ...value, workspaceId: e.target.value })} /></Field>
    <Field label={t("Qwen 推理地址", "Qwen inference endpoint")}>
      <Input value={buildQwenEndpoint(value) ?? ""} readOnly /></Field>
    <Notice tone="warning">{t("Key 必须属于所选地域与工作空间；与 Coding Plan、Web 的材料和额度不混用。", "The Key must belong to this region and workspace; do not mix Coding Plan or Web material and quota.")}</Notice>
  </fieldset>;
}
