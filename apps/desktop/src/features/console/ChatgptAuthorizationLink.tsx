import { useEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

function authorizationUrl(value: string): string | null {
  if (value.length > 8192) return null;
  try {
    const url = new URL(value);
    // Never turn an arbitrary server-supplied URL into an executable link.
    if (url.href.length > 8192 || url.origin !== "https://auth.openai.com" || url.pathname !== "/oauth/authorize"
      || url.username || url.password || url.href.includes("#")) return null;
    return url.href;
  } catch { return null; }
}

export function ChatgptAuthorizationLink({ value, busy }: { value: string; busy: boolean }) {
  const { t } = useUiLocale();
  const [failed, setFailed] = useState(false);
  const href = authorizationUrl(value);
  useEffect(() => {
    const fail = () => setFailed(true);
    window.addEventListener("gateway:oauth-browser-failed", fail);
    return () => window.removeEventListener("gateway:oauth-browser-failed", fail);
  }, []);
  const label = t("打开授权页面", "Open authorization page");
  return <>
    {href && !busy
      ? <a className="nt-btn nt-btn--secondary" href={href} target="_blank" rel="noopener noreferrer"
          onClick={() => setFailed(false)}>{label}</a>
      : <button type="button" className="nt-btn nt-btn--secondary" disabled>{label}</button>}
    {!href && <p role="alert">{t("授权链接无效，请重新登录。", "Invalid authorization URL. Sign in again.")}</p>}
    {failed && <p role="alert">{t("无法打开浏览器，请复制授权链接后手动打开。", "Cannot open the browser. Copy the authorization link and open it manually.")}</p>}
  </>;
}
