import type { Dispatch, SetStateAction } from "react";
import type { ProviderCatalogDraft } from "./providerCatalog";
import type { StorageConnection, StorageTarget } from "./providerStorageConnection";
import { CatalogField, type CatalogFieldsProps } from "./ProviderCatalogFields";

export function ProviderCatalogStorageFields({ t, workflow, update, locked, draft, setDraft, target, onBack }: CatalogFieldsProps & {
  draft: ProviderCatalogDraft; setDraft: Dispatch<SetStateAction<ProviderCatalogDraft>>; target: StorageTarget; onBack(): void;
}) {
  const connection: StorageConnection = workflow[target] ?? { type: "local", path: "" };
  const set = (patch: Partial<StorageConnection>) => update({ [target]: { ...connection, ...patch } });
  const field = (key: keyof StorageConnection, label: string) => <CatalogField name={label} key={key}>
    <input className="nt-input" disabled={locked} value={String(connection[key] ?? "")} onChange={(event) => {
      set({ [key]: event.target.value });
      if (["endpoint", "username", "bucket", "region"].includes(key)) setDraft((current) => ({ ...current,
        secrets: Object.fromEntries(Object.entries(current.secrets ?? {}).filter(([name]) => !name.startsWith(`credential_${target}_connection/`))) }));
    }} />
  </CatalogField>;
  const secret = (key: string, label: string) => {
    const path = `credential_${target}_connection/${key}`;
    return <CatalogField name={label} key={key}><input type="password" className="nt-input" autoComplete="new-password" disabled={locked}
      value={draft.secrets?.[path] ?? ""} onChange={(event) => setDraft((current) => ({ ...current, secrets: { ...current.secrets, [path]: event.target.value } }))} /></CatalogField>;
  };
  return <div className="nt-catalog-inline-editor">
    <button type="button" className="nt-btn nt-btn--outline" onClick={onBack}>{t("← 返回号池流程", "← Back to pool workflow")}</button>
    <div className="nt-catalog-settings-grid">
      <CatalogField name={t("连接方式", "Connection type")}><select className="nt-select" disabled={locked} value={connection.type} onChange={(event) => {
        update({ [target]: { type: event.target.value as StorageConnection["type"], ...(event.target.value === "s3" ? { region: "auto" } : {}) } });
        setDraft((current) => ({ ...current, secrets: Object.fromEntries(Object.entries(current.secrets ?? {}).filter(([key]) => !key.startsWith(`credential_${target}_connection/`))) }));
      }}><option value="local">{t("本地目录", "Local directory")}</option><option value="webdav">WebDAV</option><option value="s3">S3 / R2</option></select></CatalogField>
      {connection.type === "local" ? field("path", t("目录路径", "Directory path")) : <>
        {field("endpoint", t("服务地址", "Endpoint"))}
        {connection.type === "webdav" ? <>{field("directory", t("相对目录", "Relative directory"))}{field("username", t("用户名", "Username"))}{secret("password", t("密码", "Password"))}</> :
          <>{field("bucket", "Bucket")}{field("region", "Region")}{field("prefix", "Prefix")}{secret("access_key_id", "Access Key ID")}{secret("secret_access_key", "Secret Key")}{secret("session_token", "Session Token")}</>}
        <CatalogField name={t("允许 HTTP", "Allow HTTP")}><input type="checkbox" disabled={locked} checked={connection.allow_insecure_http === true} onChange={(event) => set({ allow_insecure_http: event.target.checked })} /></CatalogField>
      </>}
    </div>
  </div>;
}
