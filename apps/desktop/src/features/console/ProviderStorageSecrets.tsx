import type { StorageConnection, StorageSecretChanges, StorageSecretField } from "./providerStorageConnection";
import { storageSecretFields } from "./providerStorageConnection";
import type { TranslateFn } from "./accountsLedgerTypes";

type Props = { type: StorageConnection["type"]; configured: readonly string[]; changes: StorageSecretChanges;
  onChange: (changes: StorageSecretChanges) => void; disabled: boolean; t: TranslateFn };
const labels: Record<StorageSecretField, [string, string]> = {
  password: ["WebDAV 密码", "WebDAV password"], access_key_id: ["Access Key ID", "Access Key ID"],
  secret_access_key: ["Secret Access Key", "Secret Access Key"], session_token: ["Session Token（可选）", "Session token (optional)"],
};
export function ProviderStorageSecrets({ type, configured, changes, onChange, disabled, t }: Props) {
  return <fieldset className="nt-storage-connection__secrets" disabled={disabled}>
    <legend>{t("连接认证", "Connection authentication")}</legend>
    {storageSecretFields(type).map((field) => <label key={field}>
      <span>{t(...labels[field])}</span>
      <input className="nt-input" type="password" autoComplete="new-password"
        value={changes[field]?.operation === "replace" ? changes[field]?.value || "" : ""}
        placeholder={configured.includes(field) && changes[field]?.operation !== "clear" ? t("已配置；留空保留", "Configured; leave blank to keep") : t("未配置", "Not configured")}
        onChange={(event) => {
          const value = event.currentTarget.value;
          const next = { ...changes };
          if (value) next[field] = { operation: "replace", value }; else delete next[field];
          onChange(next);
        }} />
      {configured.includes(field) ? <span>
        <input type="checkbox" checked={changes[field]?.operation === "clear"} onChange={(event) => {
          const next = { ...changes };
          if (event.currentTarget.checked) next[field] = { operation: "clear" }; else delete next[field];
          onChange(next);
        }} /> {t("清除已保存的认证值", "Clear saved authentication value")}
      </span> : null}
    </label>)}
  </fieldset>;
}
