import { Button, Field, Notice } from "@neuro/ui";
import { useEffect, useRef, useState } from "react";

type Props = { onChange(images: string[]): void; disabled: boolean; t(zh: string, en: string): string };
const types = new Set(["image/png", "image/jpeg", "image/webp", "image/gif"]);
export const CHATAIBOT_PROBE_IMAGE_BUDGET = 180_000;

/** Local reads only. No URL fetch/preview; unmount and replacement own every FileReader. */
export function ChataibotProbeImages({ onChange, disabled, t }: Props) {
  const generation = useRef(0), readers = useRef(new Set<FileReader>());
  const [reading, setReading] = useState(false), [names, setNames] = useState<string[]>([]), [error, setError] = useState<string | null>(null);
  const stop = () => { generation.current += 1; for (const reader of readers.current) reader.abort(); readers.current.clear(); };
  useEffect(() => () => stop(), []);
  const read = (file: File) => new Promise<string>((resolve, reject) => {
    const reader = new FileReader(); readers.current.add(reader);
    const done = () => { readers.current.delete(reader); reader.onload = null; reader.onerror = null; reader.onabort = null; };
    reader.onload = () => { const value = reader.result; done(); typeof value === "string" ? resolve(value) : reject(new Error("Image read failed.")); };
    reader.onerror = () => { done(); reject(new Error("Image read failed.")); };
    reader.onabort = () => { done(); reject(new Error("Image read cancelled.")); };
    reader.readAsDataURL(file);
  });
  const select = async (files: File[]) => {
    stop(); const current = generation.current;
    onChange([]); setNames([]); setError(null); setReading(false);
    if (!files.length) return;
    if (files.length > 4 || files.some(file => !types.has(file.type) || file.size === 0) || files.reduce((n, file) => n + file.size, 0) > 135_000) {
      setError(t("请选择 1–4 张 PNG/JPEG/WebP/GIF 图片，文件合计不超过 135000 字节。", "Choose 1–4 PNG/JPEG/WebP/GIF images, at most 135000 bytes combined.")); return;
    }
    setReading(true);
    try {
      const images = await Promise.all(files.map(read));
      if (current !== generation.current) return;
      if (images.reduce((n, image) => n + image.length, 0) > CHATAIBOT_PROBE_IMAGE_BUDGET) throw new Error(t("图片编码总量超过测试预算。", "Encoded images exceed the test budget."));
      setNames(files.map(file => file.name)); onChange(images);
    } catch (cause) {
      if (current === generation.current) { for (const reader of readers.current) reader.abort(); setError(cause instanceof Error ? cause.message : String(cause)); }
    } finally { if (current === generation.current) setReading(false); }
  };
  return <>
    <Field label={t("源图片", "Source images")}><input type="file" accept={[...types].join(",")} multiple disabled={disabled} onChange={event => {
      const files = [...(event.target.files ?? [])]; event.target.value = ""; void select(files);
    }} /></Field>
    {reading ? <Notice>{t("正在读取图片", "Reading images")}</Notice> : null}
    {names.length ? <div>{names.join(" · ")} <Button disabled={disabled} onClick={() => { stop(); onChange([]); setNames([]); setReading(false); setError(null); }}>{t("清除图片", "Clear images")}</Button></div> : null}
    {error ? <Notice tone="danger">{error}</Notice> : null}
  </>;
}
