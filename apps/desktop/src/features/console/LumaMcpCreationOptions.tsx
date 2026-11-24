import { Checkbox, Field, Input, Select, Textarea } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { newMcpDraft, type McpDraft, type McpModel } from "./lumaMcpCreationDraft";
import { isMcpVideo, mcpModels } from "./lumaMcpCreationModels";
import { LumaMcpImageParameters } from "./LumaMcpImageParameters";

export function LumaMcpCreationOptions({ draft, disabled, onChange }: {
  draft: McpDraft; disabled: boolean; onChange: (value: McpDraft) => void;
}) {
  const { t } = useUiLocale();
  const video = isMcpVideo(draft.model), profile = mcpModels[draft.model];
  const thirdParty = !video && draft.model !== "luma_uni_1";
  const hdrModel = draft.model === "luma_ray3_14_hdr";
  const edit = draft.operation === "edit";
  const reframe = draft.operation === "reframe";
  const update = (key: keyof McpDraft, value: string) => onChange({ ...draft, [key]: value });
  const select = (key: Exclude<keyof McpDraft, "reuseEnabled">, label: string, values: string[]) => <Field label={label}>
    <Select value={draft[key]} disabled={disabled} onChange={e => update(key, e.target.value)}>
      <option value="">{t("默认", "Default")}</option>{values.map(value => <option key={value} value={value}>{value}</option>)}
    </Select></Field>;
  const input = (key: Exclude<keyof McpDraft, "reuseEnabled">, label: string, placeholder?: string) => <Field label={label}>
    <Input value={draft[key]} disabled={disabled} onChange={e => update(key, e.target.value)} placeholder={placeholder} /></Field>;
  return <>
    <Field label={t("创作模型", "Creation model")}><Select value={draft.model} disabled={disabled}
      onChange={e => onChange({ ...newMcpDraft(e.target.value as McpModel),
        ...(draft.reuseEnabled && draft.operation === "generate" && !video && !isMcpVideo(e.target.value as McpModel)
          ? { reuseEnabled: true, reuseTask: draft.reuseTask, referenceAssets: draft.referenceAssets } : {}) })}>
      {(Object.entries(mcpModels) as [McpModel, typeof profile][]).map(([id, model]) => <option key={id} value={id}>{model.label}</option>)}
    </Select></Field>
    {!thirdParty && !hdrModel && <Field label={t("创作操作", "Creation operation")}><Select value={draft.operation} disabled={disabled}
      onChange={e => onChange(newMcpDraft(draft.model, e.target.value as McpDraft["operation"]))}>
      <option value="generate">{t("生成", "Generate")}</option><option value="edit">{t("编辑", "Edit")}</option>
      {video && <option value="reframe">{t("扩画", "Reframe")}</option>}
    </Select></Field>}
    {(!video || edit || reframe) && <Checkbox checked={draft.reuseEnabled} disabled={disabled}
      label={t("使用已保存任务的素材", "Use assets from a saved task")}
      onChange={e => onChange({ ...draft, reuseEnabled: e.target.checked, source: "", reuseTask: "", sourceAsset: "", referenceAssets: "" })} />}
    {draft.reuseEnabled && <>
      {input("reuseTask", t("素材来源任务 ID", "Source task ID"))}
      {(edit || reframe) && input("sourceAsset", t("源素材 ID", "Source asset ID"))}
      {!video && <Field label={t("参考图片 ID（每行一个）", "Reference image IDs (one per line)")}><Textarea value={draft.referenceAssets}
        disabled={disabled} maxLength={1040} onChange={e => update("referenceAssets", e.target.value)} /></Field>}
    </>}
    {(edit || reframe) && !draft.reuseEnabled && input("source", t("源素材 URL", "Source URL"), "https://…")}
    {video && (edit || reframe) && !draft.reuseEnabled && select("sourceType", t("源视频类型", "Source video MIME type"), ["video/mp4", "video/webm", "video/quicktime"])}
    {!edit && profile.ratios.length > 0 && select("ratio", t("画面比例", "Aspect ratio"), profile.ratios)}
    {!video && <>
      <Field label={t("参考图片 URL（每行一个）", "Reference image URLs (one per line)")}><Textarea value={draft.references}
        disabled={disabled} maxLength={24576} onChange={e => update("references", e.target.value)} /></Field>
      {!thirdParty && <>{select("style", t("图片风格", "Image style"), ["auto", "manga"])}
        {select("format", t("图片格式", "Image format"), ["png", "jpeg"])}</>}
      {thirdParty && <LumaMcpImageParameters draft={draft} disabled={disabled} onChange={onChange} />}
    </>}
    {video && <>
      {select("resolution", t("视频分辨率", "Video resolution"), hdrModel ? ["720p", "1080p"] : ["360p", "540p", "720p", "1080p"])}
      {!edit && select("duration", t("视频时长", "Video duration"), hdrModel ? ["5s"] : ["5s", "10s"])}
      {!reframe && !hdrModel && (draft.model === "luma_ray3_2" || edit) && select("hdr", "HDR", ["false", "true"])}
      {draft.model === "luma_ray3_14" && !edit && !reframe && select("loopVideo", t("循环视频", "Loop video"), ["false", "true"])}
      {!reframe && input("startFrame", t("起始帧 URL", "Start frame URL"), "https://…")}
      {!edit && !reframe && <>
        {input("endFrame", t("结束帧 URL", "End frame URL"), "https://…")}
        {draft.model === "luma_ray3_2" && <Field label={t("关键帧（每行：帧序号 URL）", "Keyframes (one index and URL per line)")}><Textarea value={draft.keyframes}
          disabled={disabled} maxLength={24576} placeholder="0 https://…" onChange={e => update("keyframes", e.target.value)} /></Field>}
      </>}
      {edit && <>
        {select("strength", t("重绘强度", "Edit strength"), ["adhere_1", "adhere_2", "adhere_3", "flex_1", "flex_2", "flex_3", "reimagine_1", "reimagine_2", "reimagine_3"])}
        {select("autoControls", t("自动控制", "Automatic controls"), ["false", "true"])}
      </>}
      {reframe && input("position", t("源画面位置（x, y, 宽, 高）", "Source position (x, y, width, height)"), "0, 0, 1, 1")}
    </>}
  </>;
}
