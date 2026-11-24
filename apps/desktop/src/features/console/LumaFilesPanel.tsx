import { useEffect, useRef, useState } from "react";
import { Button, Field, Input, Notice, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useLumaFiles, type LumaFilesProps } from "./useLumaFiles";

export function LumaFilesPanel(props: LumaFilesProps) {
  const { t } = useUiLocale();
  const files = useLumaFiles(props);
  const [upload, setUpload] = useState<File | null>(null);
  const [purpose, setPurpose] = useState<"input" | "reference">("input");
  const [expiresAt, setExpiresAt] = useState("");
  const [userId, setUserId] = useState("");
  const [copied, setCopied] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const copyTarget = useRef({ scope: files.scope, id: files.selected });
  copyTarget.current = { scope: files.scope, id: files.selected };
  useEffect(() => { setUpload(null); setPurpose("input"); setExpiresAt(""); setUserId(""); setCopied(false); if (input.current) input.current.value = ""; }, [files.scope]);
  useEffect(() => setCopied(false), [files.selected, files.file?.state]);
  const disabled = !files.enabled || files.busy;
  const stateLabel = { pending: t("处理中", "Processing"), ready: t("可用", "Ready"), failed: t("处理失败", "Failed"), deleted: t("已删除", "Deleted") };
  return <section className="n-stack" aria-label={t("Luma 文件", "Luma files")}>
    <Field label={t("上传文件（最大 5 GiB）", "Upload file (up to 5 GiB)")}>
      <input ref={input} type="file" accept="image/*,video/*" disabled={disabled}
        onChange={(event) => setUpload(event.target.files?.[0] ?? null)} />
    </Field>
    <Field label={t("文件用途", "File purpose")}><Select disabled={disabled} value={purpose}
      onChange={(event) => setPurpose(event.target.value as "input" | "reference")}>
      <option value="input">{t("输入素材", "Input")}</option><option value="reference">{t("参考素材", "Reference")}</option>
    </Select></Field>
    <Field label={t("过期时间（可选）", "Expiration (optional)")}><Input type="datetime-local" disabled={disabled} value={expiresAt} onChange={(event) => setExpiresAt(event.target.value)} /></Field>
    <Field label={t("用户标记（可选）", "User tag (optional)")}><Input disabled={disabled} value={userId} maxLength={256} onChange={(event) => setUserId(event.target.value)} /></Field>
    <div className="nt-actions">
      <Button disabled={disabled || !upload} onClick={() => upload && void files.perform({ action: "upload", file: upload, purpose, expiresAt, userId })}>{t("上传文件", "Upload file")}</Button>
      <Button disabled={disabled} onClick={() => void files.perform({ action: "list" })}>{t("读取文件列表", "Read files")}</Button>
      {files.page?.cursor && <Button disabled={disabled} onClick={() => void files.perform({ action: "list", cursor: files.page!.cursor! })}>{t("下一页", "Next page")}</Button>}
      {files.busy && <Button onClick={files.stop}>{t("停止等待", "Stop waiting")}</Button>}
    </div>
    {files.error && <Notice tone="danger">{files.error === "file" ? t("请选择不超过 5 GiB 的非空图片或视频，并检查文件名、过期时间和用户标记。", "Choose a nonempty image or video up to 5 GiB; check its name, expiration and user tag.")
      : t("文件操作未完成。上传或删除可能已生效，请先刷新列表核对。", "File operation did not complete. An upload or deletion may have taken effect; refresh the list first.")}</Notice>}
    {files.page?.rows.length === 0 && <Notice>{t("没有文件", "No files")}</Notice>}
    {!!files.page?.rows.length && <Field label={t("已上传文件", "Uploaded files")}><Select disabled={disabled} value={files.selected} onChange={(event) => files.select(event.target.value)}>
      {files.page.rows.map((file) => <option key={file.id} value={file.id}>{file.filename ?? file.id} · {stateLabel[file.state]}</option>)}
    </Select></Field>}
    {files.file && <>
      <div role="status">{stateLabel[files.file.state]} · {files.file.mime_type} · {files.file.size_bytes} B</div>
      {files.file.expires_at && <div>{t("过期时间", "Expiration")} · {files.file.expires_at}</div>}
      {files.file.user_id && <div>{t("用户标记", "User tag")} · {files.file.user_id}</div>}
      <Field label="file_id"><Input readOnly value={files.file.id} /></Field>
      <div className="nt-actions">
        <Button disabled={disabled || files.file.state !== "ready"} onClick={() => {
          const target = copyTarget.current;
          void navigator.clipboard?.writeText(target.id).then(() => {
            if (copyTarget.current.scope === target.scope && copyTarget.current.id === target.id) setCopied(true);
          }, () => setCopied(false));
        }}>{copied ? t("已复制", "Copied") : t("复制 file_id", "Copy file_id")}</Button>
        <Button disabled={disabled} onClick={() => void files.perform({ action: "get", id: files.file!.id })}>{t("刷新文件状态", "Refresh file")}</Button>
        <Button disabled={disabled} onClick={() => files.setConfirmDelete(files.file!.id)}>{t("删除文件", "Delete file")}</Button>
      </div>
      {files.confirmDelete === files.file.id && <div className="n-stack">
        <Notice tone="danger">{t(`删除 ${files.file.filename ?? files.file.id} 后将无法继续引用，且无法通过 API 恢复。`, `Deleting ${files.file.filename ?? files.file.id} prevents further references and cannot be undone through the API.`)}</Notice>
        <div className="nt-actions"><Button disabled={disabled} onClick={() => void files.perform({ action: "delete", id: files.file!.id })}>{t("确认删除文件", "Confirm deletion")}</Button>
          <Button onClick={() => files.setConfirmDelete(null)}>{t("取消删除", "Keep file")}</Button></div>
      </div>}
    </>}
  </section>;
}
