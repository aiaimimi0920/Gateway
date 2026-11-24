import { Field, Input, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { McpDraft } from "./lumaMcpCreationDraft";
import { imageParameterFields, mcpModels } from "./lumaMcpCreationModels";

export function LumaMcpImageParameters({ draft, disabled, onChange }: {
  draft: McpDraft; disabled: boolean; onChange: (value: McpDraft) => void;
}) {
  const { t } = useUiLocale();
  const labels: Record<keyof typeof imageParameterFields, string> = {
    resolution: t("图片分辨率", "Image resolution"), quality: t("图片质量", "Image quality"),
    output_format: t("图片格式", "Image format"), background: t("图片背景", "Image background"),
    size: t("图片尺寸", "Image size"), thinking_level: t("推理程度", "Thinking level"),
    enable_search: t("搜索增强", "Search grounding"), temperature: t("温度", "Temperature"),
    magic_prompt: t("提示词增强", "Magic prompt"),
  };
  return <>{Object.entries(mcpModels[draft.model].parameters).map(([name, schema]) => {
    const key = name as keyof typeof imageParameterFields, field = imageParameterFields[key];
    const values = schema.enum ?? (schema.type === "boolean" ? ["false", "true"] : null);
    return <Field key={key} label={labels[key]}>{values
      ? <Select value={draft[field]} disabled={disabled} onChange={e => onChange({ ...draft, [field]: e.target.value })}>
        <option value="">{t("默认", "Default")}</option>{values.map(value => <option key={value} value={value}>{value}</option>)}
      </Select>
      : <Input value={draft[field]} disabled={disabled} type={schema.type === "number" ? "number" : "text"}
        min={schema.minimum} max={schema.maximum} step={schema.type === "number" ? "any" : undefined}
        maxLength={32} placeholder={key === "size" ? "1K / 1.5K / 2K / 1536x1024" : undefined}
        onChange={e => onChange({ ...draft, [field]: e.target.value })} />}</Field>;
  })}</>;
}
