import { Button, Checkbox, Dialog, Field, Input, Notice, Select } from "@neuro/ui";
import { useId, useRef, useState } from "react";
import type { TranslateFn } from "./accountsLedgerTypes";
import { ProviderStorageSecrets } from "./ProviderStorageSecrets";
import { resolveProviderCardDialogFocus } from "./providerCardDialogFocus";
import { storageConnectionIdentity, storageSecretFields, validateStorageConnection,
  type StorageConnection, type StorageSecretChanges, type StorageTarget } from "./providerStorageConnection";

type Props = {
  providerLabel: string; target: StorageTarget; connection?: StorageConnection; defaultPath: string;
  configuredSecrets: readonly string[]; disabled: boolean; onCancel: () => void;
  onSave: (connection: StorageConnection, secrets: StorageSecretChanges) => boolean; t: TranslateFn;
};

/** Only non-secret connection metadata enters the route JSON; auth stays in secret patches. */
export function ProviderStorageConnectionDialog({ providerLabel, target, connection, defaultPath,
  configuredSecrets, disabled, onCancel, onSave, t,
}: Props) {
  const initial = connection ?? { type: "local" as const, path: defaultPath };
  const [draft, setDraft] = useState<StorageConnection>({ ...initial });
  const [secrets, setSecrets] = useState<StorageSecretChanges>({});
  const [error, setError] = useState("");
  const protocolRef = useRef<HTMLSelectElement | null>(null);
  const errorId = useId();
  const submitting = useRef(false);
  const identityChanged = storageConnectionIdentity(draft) !== storageConnectionIdentity(connection);
  const title = t(`${providerLabel} ${target === "storage" ? "存储连接" : "归档连接"}`, `${providerLabel} ${target} connection`);
  const update = (field: keyof StorageConnection, value: string) => {
    setDraft((current) => ({ ...current, [field]: value }));
    setError("");
  };
  const textField = (field: keyof StorageConnection, label: string, placeholder?: string) => <Field key={field} label={label}>
    <Input value={String(draft[field] ?? "")} placeholder={placeholder}
      onChange={(event) => update(field, event.currentTarget.value)} autoComplete="off" />
  </Field>;
  const save = () => {
    if (disabled || submitting.current) return;
    if (draft.type === "webdav" && secrets.password?.operation === "replace" && !draft.username?.trim()) {
      setError(t("WebDAV 密码需要同时填写用户名。", "A WebDAV password requires a username."));
      return;
    }
    const invalid = validateStorageConnection(draft);
    if (invalid) { setError(invalid); return; }
    if (identityChanged && configuredSecrets.some((field) =>
      storageSecretFields(draft.type).includes(field as keyof StorageSecretChanges) && !secrets[field as keyof StorageSecretChanges])) {
      setError(t("连接目标已变化，请替换或明确清除已有认证值，不能自动沿用。", "Connection identity changed. Replace or explicitly clear saved authentication."));
      return;
    }
    submitting.current = true;
    try { if (onSave(draft, secrets)) onCancel(); }
    finally { submitting.current = false; }
  };
  return <Dialog open title={title} className="nt-storage-connection"
    description={t("地址位于 Gateway 服务器或所选云服务。保存仅更新配置；不会创建云资源。", "This location belongs to the Gateway server or selected cloud service. Saving updates configuration, not cloud resources.")}
    initialFocusRef={protocolRef} resolveReturnFocus={resolveProviderCardDialogFocus}
    aria-describedby={error ? errorId : undefined} onOpenChange={(open) => { if (!open) onCancel(); }}>
        <form className="nt-storage-connection__form" onSubmit={(event) => { event.preventDefault(); save(); }}>
          <div className="nt-storage-connection__body">
          <fieldset disabled={disabled} className="nt-storage-connection__fields">
            <Field label={t("存储协议", "Storage protocol")}>
              <Select ref={protocolRef} value={draft.type} onChange={(event) => {
                const type = event.currentTarget.value as StorageConnection["type"];
                setDraft(type === "local" ? { type, path: defaultPath } : type === "s3"
                  ? { type, endpoint: "", bucket: "", region: "auto", prefix: "" }
                  : { type, endpoint: "", directory: "", username: "" });
                setSecrets({}); setError("");
              }}><option value="local">{t("本地 / 已挂载目录", "Local / mounted directory")}</option><option value="webdav">WebDAV</option><option value="s3">S3 / R2</option></Select>
            </Field>
            {draft.type === "local" ? textField("path", t("目录路径", "Directory path")) : <>
              {textField("endpoint", t("服务地址", "Endpoint"), "https://")}
              {draft.type === "webdav" ? <>
                {textField("directory", t("相对目录", "Relative directory"), "gateway")}
                {textField("username", t("WebDAV 用户名", "WebDAV username"))}
                <Notice tone="warning" role="note">{t("未确认服务端原子删除能力的 WebDAV 连接仅启用读取和安全归档创建，归档清空受限。", "Without confirmed server-side atomic deletion, WebDAV supports reads and safe archive creation; archive purge is restricted.")}</Notice>
              </> : <>
                {textField("bucket", "Bucket")}{textField("region", "Region", "auto")}
                {textField("prefix", t("对象前缀", "Object prefix"), "gateway")}
                <Notice tone="warning" role="note">{t("R2 归档清空受限；未确认的 S3 兼容服务仅启用读取，归档创建与清空按已核验能力开放。", "R2 archive purge is restricted. Unverified S3-compatible services are read-only; archive creation and purge require verified capabilities.")}</Notice>
              </>}
              <p className="nt-copy">{t("程序会在目录或前缀内按服务商隔离补号素材和归档，不会清空整个 Bucket。", "Refill and archive objects use a provider-isolated namespace within this directory or prefix. Purge does not empty the bucket.")}</p>
              {draft.endpoint?.toLowerCase().startsWith("http:") ? <Checkbox checked={draft.allow_insecure_http === true}
                label={t("我理解 HTTP 会明文发送认证凭据和账号数据，仍允许此连接（建议 HTTPS）", "I understand HTTP sends authentication and account data unencrypted, and allow this connection (HTTPS recommended)")}
                onChange={(event) => { const checked = event.currentTarget.checked; setDraft((current) => ({ ...current, allow_insecure_http: checked })); }} /> : null}
            </>}
          </fieldset>
          {identityChanged && configuredSecrets.length > 0 && draft.type !== "local" ? <Notice tone="warning" role="note">{t("连接身份已改变，原认证不会自动沿用", "Connection identity changed; saved authentication is not automatically reused")}</Notice> : null}
          {draft.type !== "local" ? <ProviderStorageSecrets type={draft.type} configured={configuredSecrets}
            changes={secrets} onChange={setSecrets} disabled={disabled} t={t} /> : <p className="nt-copy">{t("本地文件不使用连接密码加密。旧补号程序密码仍保持原用途。", "Local files are not encrypted with a connection password. The legacy refill-worker password keeps its original purpose.")}</p>}
          {error ? <Notice id={errorId} tone="danger">{error}</Notice> : null}
          </div>
          <div className="n-dialog-actions"><Button onClick={onCancel}>{t("取消", "Cancel")}</Button>
            <Button variant="primary" type="submit" disabled={disabled}>{t("保存连接", "Save connection")}</Button>
          </div>
        </form>
  </Dialog>;
}
