import { ShieldCheck } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { isUsableManagementToken } from "../../session/managementTokenRotation";
import { SettingsSection } from "./SettingsSection";
import "./ManagementSecuritySettings.css";

export function ManagementSecuritySettings() {
  const { t } = useUiLocale();
  const host = useGatewayHost();
  const session = useManagementSession();
  const [newToken, setNewToken] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [acknowledged, setAcknowledged] = useState(false);
  const [pending, setPending] = useState(false);
  const [notice, setNotice] = useState<"success" | "failed" | null>(null);
  const generation = useRef(0);
  const inFlight = useRef(false);
  const environmentOverride = session.bootstrapStatus?.environmentOverride === true;
  const available = session.phase === "authenticated" && session.bootstrapStatus !== null && !environmentOverride;
  const locked = !available || pending || session.busy;
  const token = newToken.trim();
  const mismatch = confirmation.length > 0 && token !== confirmation.trim();
  const unchanged = token.length > 0 && token === session.managementToken;
  const invalidToken = token.length > 0 && !isUsableManagementToken(token);
  const canSubmit = !locked && isUsableManagementToken(token) && !unchanged && token === confirmation.trim() && acknowledged;

  useEffect(() => {
    generation.current += 1;
    inFlight.current = false;
    setNewToken(""); setConfirmation(""); setAcknowledged(false); setPending(false); setNotice(null);
    return () => { generation.current += 1; };
  }, [host.apiOrigin]);

  const submit = async () => {
    if (!canSubmit || inFlight.current) return;
    const current = generation.current;
    inFlight.current = true;
    setPending(true); setNotice(null);
    try {
      await session.rotate(token);
      if (generation.current !== current) return;
      setNewToken(""); setConfirmation(""); setAcknowledged(false); setNotice("success");
    } catch {
      if (generation.current === current) setNotice("failed");
    } finally {
      if (generation.current === current) { inFlight.current = false; setPending(false); }
    }
  };

  return <SettingsSection label={t("管理安全", "Management security")} icon={<ShieldCheck size={18} />}>
    <form className="nt-stack" onSubmit={(event) => { event.preventDefault(); void submit(); }}>
      <p>{t("更新登录管理后台的密钥，不影响项目 API 访问密钥。旧管理密钥会立即失效，其他客户端需要重新登录。", "Change the console sign-in token, not project API access keys. The old management token stops working immediately; other clients must sign in again.")}</p>
      {environmentOverride
        ? <p role="status">{t("管理密钥由环境变量托管，不能在界面修改。请在部署配置中更新。", "The management token is controlled by an environment variable. Update the deployment configuration instead.")}</p>
        : <>
          <div className="nt-grid nt-grid--2">
            <label className="nt-field"><span>{t("新管理密钥", "New management token")}</span>
              <input className="nt-input" type="password" autoComplete="new-password" maxLength={4096} required
                value={newToken} disabled={locked} onChange={(event) => { setNewToken(event.target.value); setNotice(null); }} />
            </label>
            <label className="nt-field"><span>{t("再次输入新密钥", "Confirm new token")}</span>
              <input className="nt-input" type="password" autoComplete="new-password" maxLength={4096} required
                value={confirmation} disabled={locked} onChange={(event) => { setConfirmation(event.target.value); setNotice(null); }} />
            </label>
          </div>
          {mismatch && <p role="alert">{t("两次输入的密钥不一致。", "The tokens do not match.")}</p>}
          {invalidToken && <p role="alert">{t("请使用不含空格的 ASCII 字母、数字或符号，最多 4096 个字符。", "Use ASCII letters, digits or symbols without spaces, up to 4096 characters.")}</p>}
          {unchanged && <p role="alert">{t("新密钥必须与当前密钥不同。", "The new token must differ from the current token.")}</p>}
          <label className="nt-management-security__ack">
            <input type="checkbox" checked={acknowledged} disabled={locked} onChange={(event) => setAcknowledged(event.target.checked)} />
            <span>{t("我已保存新密钥，并了解旧密钥将立即失效。", "I have saved the new token and understand that the old token will stop working immediately.")}</span>
          </label>
          <div className="nt-actions"><button className="nt-btn nt-btn--danger" type="submit" disabled={!canSubmit}>
            {pending ? t("正在更新…", "Updating…") : t("更新管理密钥", "Update management token")}
          </button></div>
        </>}
      {notice === "success" && <p className="nt-alert nt-alert--success" role="status">{t("管理密钥已更新，当前会话已切换到新密钥。", "Management token updated. This session now uses the new token.")}</p>}
      {notice === "failed" && <p className="nt-alert nt-alert--danger" role="alert">{t("未能确认密钥更新结果。请保留新旧密钥，检查连接与部署配置后再登录确认。", "The token update could not be confirmed. Keep both tokens and check the connection and deployment configuration before signing in again.")}</p>}
    </form>
  </SettingsSection>;
}
