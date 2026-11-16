import { Field, Input, Select, Textarea } from "@neuro/ui";

export type MediaProbeSource = { kind: "url" | "generation_id" | "file_id" | "data"; value: string; mime: string };
export const emptyMediaProbeSource = (): MediaProbeSource => ({ kind: "url", value: "", mime: "image/png" });
export function mediaProbeSource(value: MediaProbeSource): Record<string, string> | null {
  if (value.kind === "data") return value.value.length > 0 && value.value.length <= 180_000
    && /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value.value)
    ? { data: value.value, media_type: value.mime } : null;
  if (value.kind !== "url") return /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(value.value)
    ? { [value.kind]: value.value } : null;
  try {
    const url = new URL(value.value);
    return value.value.length <= 8192 && url.protocol === "https:" && !url.username && !url.password && !url.port && !url.hash
      ? { url: value.value } : null;
  } catch { return null; }
}
export function LumaMediaProbeSource({ value, onChange, disabled, t }: {
  value: MediaProbeSource; onChange(value: MediaProbeSource): void; disabled: boolean; t(zh: string, en: string): string;
}) {
  return <>
    <Field label={t("源图片类型", "Source image type")}><Select value={value.kind} disabled={disabled}
      onChange={e => onChange({ ...emptyMediaProbeSource(), kind: e.target.value as MediaProbeSource["kind"] })}>
      <option value="url">URL</option><option value="generation_id">Generation ID</option><option value="file_id">File ID</option><option value="data">Base64</option>
    </Select></Field>
    {value.kind === "data" ? <>
      <Field label={t("图片 MIME", "Image MIME")}><Select value={value.mime} disabled={disabled} onChange={e => onChange({ ...value, mime: e.target.value })}>
        {["image/png", "image/jpeg", "image/webp", "image/gif"].map(mime => <option key={mime}>{mime}</option>)}
      </Select></Field>
      <Field label={t("图片 Base64（最多 180000 字符）", "Image Base64 (up to 180000 characters)")}><Textarea value={value.value} disabled={disabled} rows={3}
        maxLength={180_000} onChange={e => onChange({ ...value, value: e.target.value })} /></Field>
    </> : <Field label={t("源图片引用", "Source image reference")}><Input value={value.value} disabled={disabled}
      maxLength={value.kind === "url" ? 8192 : 36} onChange={e => onChange({ ...value, value: e.target.value })} /></Field>}
  </>;
}
