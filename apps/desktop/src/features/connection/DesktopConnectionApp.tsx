import { LanguageToggleButton } from "../../i18n/LanguageToggleButton";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useDesktopConnection } from "./useDesktopConnection";

export function DesktopConnectionApp() {
  const { t } = useUiLocale();
  const { settings, setSettings, busy, error, address, connect } = useDesktopConnection();
  return (
    <main className="auth-page" aria-labelledby="connection-title">
      <div className="auth-page__toolbar"><LanguageToggleButton /></div>
      <form className="auth-panel" onSubmit={(event) => { event.preventDefault(); void connect(settings); }}>
        <h1 id="connection-title">Gateway</h1>
        <p>{t("桌面与浏览器使用同一套管理后台。", "The desktop and browser use the same management console.")}</p>
        <label htmlFor="connection-mode">{t("连接方式", "Connection mode")}</label>
        <select id="connection-mode" value={settings.mode} disabled={busy}
          onChange={(event) => setSettings({ ...settings, mode: event.currentTarget.value === "existing" ? "existing" : "local" })}>
          <option value="local">{t("本机 Gateway（自动运行）", "Local Gateway (automatic)")}</option>
          <option value="existing">{t("连接已有 Gateway", "Connect to an existing Gateway")}</option>
        </select>
        {settings.mode === "existing" ? <>
          <label htmlFor="gateway-server">{t("服务器地址", "Server address")}</label>
          <input id="gateway-server" type="url" placeholder="https://gateway.example.com" required
            value={settings.serverUrl} disabled={busy} autoComplete="url"
            onChange={(event) => setSettings({ ...settings, serverUrl: event.currentTarget.value })} />
          <p>{t("使用服务器的管理密钥登录。此模式不会启动本地后端，也不会停止已有服务器。", "Sign in with the server's management token. This mode never starts a local backend or stops the existing server.")}</p>
        </> : <p>{t("自动启动随附的 Gateway，配置保存在用户目录。新安装默认管理密钥：11011101。", "Starts the bundled Gateway automatically and keeps configuration in your user directory. New installation management token: 11011101.")}</p>}
        {busy && <p role="status">{t("正在连接 Gateway，请稍候…", "Connecting to Gateway…")}</p>}
        {error && <p role="alert">{error}</p>}
        {address && <p role="status">{t("当前后台：", "Current console: ")}{address}</p>}
        <button type="submit" disabled={busy}>{busy ? t("连接中…", "Connecting…") : t("打开管理后台", "Open console")}</button>
        <p>{t("点击管理窗口右上角的“连接设置”，或按 Ctrl+Shift+G 可切换服务器。关闭管理窗口会退出客户端。", "Use Connection settings at the top right, or Ctrl+Shift+G, to switch servers. Closing the console quits the client.")}</p>
      </form>
    </main>
  );
}
