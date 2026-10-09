import { Eye, EyeOff } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { isUsableManagementToken } from "../../session/managementTokenRotation";
import type { ManagementKey } from "./managementKeysApi";

type Props = {
  entry?: ManagementKey;
  locked: boolean;
  onCancel: () => void;
  onSave: (name: string, token: string) => void;
};

export function ManagementKeyEditor({ entry, locked, onCancel, onSave }: Props) {
  const { t } = useUiLocale();
  const [name, setName] = useState(entry?.name ?? "");
  const [token, setToken] = useState("");
  const [visible, setVisible] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { input.current?.focus(); }, []);
  const valid = !!name.trim() && ((!token && !!entry) || isUsableManagementToken(token.trim()));
  const revealLabel = visible ? t("隐藏输入的密钥", "Hide entered key") : t("显示输入的密钥", "Show entered key");
  return <form className="nt-management-keys__editor" aria-label={entry ? t("编辑管理密钥", "Edit management key") : t("添加管理密钥", "Add management key")}
    onKeyDown={event => { if (event.key === "Escape" && !locked) { event.preventDefault(); onCancel(); } }}
    onSubmit={event => { event.preventDefault(); if (!locked && valid) onSave(name.trim(), token.trim()); }}>
    <div className="nt-management-keys__row">
      <input ref={input} className="nt-input" aria-label={t("密钥名称", "Key name")} placeholder={t("密钥名称", "Key name")}
        maxLength={80} required disabled={locked} value={name} onChange={event => setName(event.target.value)} />
      <input className="nt-input nt-management-keys__secret" aria-label={t("管理密钥", "Management key")} type={visible ? "text" : "password"}
        placeholder={entry ? t("留空保留原密钥", "Leave blank to keep the key") : t("输入管理密钥", "Enter management key")}
        autoComplete="new-password" maxLength={4096} required={!entry} disabled={locked} value={token} onChange={event => setToken(event.target.value)} />
      <div className="nt-management-keys__actions">
        <button type="button" className="nt-btn nt-btn--secondary nt-management-keys__eye" disabled={locked} aria-label={revealLabel} title={revealLabel}
          aria-pressed={visible} onClick={() => setVisible(value => !value)}>{visible ? <EyeOff size={16} /> : <Eye size={16} />}</button>
        <button type="submit" className="nt-btn nt-btn--primary" disabled={locked || !valid}>{t("保存", "Save")}</button>
        <button type="button" className="nt-btn nt-btn--secondary" disabled={locked} onClick={onCancel}>{t("取消", "Cancel")}</button>
      </div>
    </div>
    {token && !isUsableManagementToken(token.trim()) && <p role="alert" className="nt-management-keys__message">{t("请使用不含空格的 ASCII 字母、数字或符号。", "Use printable ASCII without spaces.")}</p>}
    {entry?.current && token && <p className="nt-management-keys__hint">{t("替换当前密钥后需重新登录，请保存新密钥。", "Replacing the current key signs you out. Keep the new key.")}</p>}
    {!entry && <p className="nt-management-keys__hint">{t("请保存新密钥。首次添加后由此列表管理认证，环境变量不再覆盖列表。", "Keep the new key. After the first addition, this list owns authentication instead of the environment variable.")}</p>}
  </form>;
}
