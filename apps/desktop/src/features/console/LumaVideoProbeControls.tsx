import { Button, Checkbox, Field, Input, Notice, Select } from "@neuro/ui";
import { LumaMediaProbeSource } from "./LumaMediaProbeSource";
import { emptyLumaGuide, LUMA_SIGNALS, type LumaVideoControlsDraft } from "./lumaVideoProbeDraft";

type Props = { value: LumaVideoControlsDraft; onChange(value: LumaVideoControlsDraft): void;
  operation: string; resolution: string; disabled: boolean; t(zh: string, en: string): string };

/** Visible controls match the supported REST profile; draft values are never saved to accounts. */
export function LumaVideoProbeControls({ value, onChange, operation, resolution, disabled, t }: Props) {
  if (operation === "video_reframe") return <>
    <Checkbox label={t("自定义扩画位置", "Custom reframe position")} checked={value.positioned} disabled={disabled}
      onChange={event => onChange({ ...value, positioned: event.target.checked })} />
    {value.positioned ? ([
      ["x_norm", "水平位置", "Horizontal position"], ["y_norm", "垂直位置", "Vertical position"],
      ["w_norm", "相对宽度", "Relative width"], ["h_norm", "相对高度", "Relative height"],
    ] as const).map(([key, zh, en]) => <Field key={key} label={t(zh, en)}><Input type="number" step="0.01" min={key === "x_norm" || key === "y_norm" ? -2 : 0} max={2}
      value={value.position[key]} disabled={disabled} onChange={event => onChange({ ...value, position: { ...value.position, [key]: event.target.value } })} /></Field>) : null}
  </>;
  if (operation !== "video_edit") return null;
  return <>
    <Checkbox label="HDR" checked={value.hdr} disabled={disabled} onChange={event => onChange({ ...value, hdr: event.target.checked })} />
    {value.hdr && !["720p", "1080p"].includes(resolution) ? <Notice tone="warning">{t("HDR 需要 720p 或 1080p。", "HDR requires 720p or 1080p.")}</Notice> : null}
    <Checkbox label={t("自定义约束", "Custom conditioning")} checked={value.manualSignals} disabled={disabled}
      onChange={event => onChange({ ...value, manualSignals: event.target.checked })} />
    {value.manualSignals ? LUMA_SIGNALS.map(([name, zh, en, tuning, tuningZh, tuningEn]) => {
      const signal = value.signals[name];
      const update = (patch: Partial<typeof signal>) => onChange({ ...value, signals: { ...value.signals, [name]: { ...signal, ...patch } } });
      return <div key={name}>
        <Field label={t(zh, en)}><Select value={signal.mode} disabled={disabled} onChange={event => update({ mode: event.target.value as typeof signal.mode })}>
          <option value="default">{t("默认", "Default")}</option><option value="enabled">{t("启用", "Enabled")}</option><option value="disabled">{t("停用", "Disabled")}</option>
        </Select></Field>
        {signal.mode === "enabled" && tuning ? <Field label={t(tuningZh, tuningEn)}>{name === "pose"
          ? <Select value={signal.tuning} disabled={disabled} onChange={event => update({ tuning: event.target.value })}><option value="">{t("默认", "Default")}</option><option value="precise">{t("精确", "Precise")}</option><option value="coarse">{t("粗略", "Coarse")}</option></Select>
          : <Input type="number" min={0} max={1} step="0.01" value={signal.tuning} disabled={disabled} onChange={event => update({ tuning: event.target.value })} />}</Field> : null}
      </div>;
    }) : null}
    <Field label={t("引导图模式", "Guide image mode")}><Select value={value.guideMode} disabled={disabled}
      onChange={event => onChange({ ...value, guideMode: event.target.value as typeof value.guideMode, guides: [emptyLumaGuide()] })}>
      <option value="none">{t("无", "None")}</option><option value="single">{t("单张引导图", "Single guide image")}</option><option value="keyframes">{t("多关键帧", "Multiple keyframes")}</option>
    </Select></Field>
    {value.guideMode !== "none" ? value.guides.map((guide, index) => {
      const update = (patch: Partial<typeof guide>) => onChange({ ...value, guides: value.guides.map((row, position) => position === index ? { ...row, ...patch } : row) });
      return <fieldset key={index} disabled={disabled}>
        <legend>{value.guideMode === "single" ? t("引导图", "Guide image") : t(`关键帧 ${index + 1}`, `Keyframe ${index + 1}`)}</legend>
        {value.guideMode === "keyframes" ? <Field label={t("源帧序号", "Source frame index")}><Input type="number" min={0} max={Number.MAX_SAFE_INTEGER} step={1} value={guide.index} disabled={disabled}
          onChange={event => update({ index: event.target.value })} /></Field> : null}
        <LumaMediaProbeSource value={guide.source} onChange={source => update({ source })} disabled={disabled} t={t} />
        {value.guideMode === "keyframes" ? <Button disabled={disabled || value.guides.length <= 1} onClick={() => onChange({ ...value, guides: value.guides.filter((_, position) => position !== index) })}>{t(`移除关键帧 ${index + 1}`, `Remove keyframe ${index + 1}`)}</Button> : null}
      </fieldset>;
    }) : null}
    {value.guideMode === "keyframes" ? <Button disabled={disabled || value.guides.length >= 64} onClick={() => onChange({ ...value, guides: [...value.guides, emptyLumaGuide()] })}>{t("添加关键帧", "Add keyframe")}</Button> : null}
  </>;
}
