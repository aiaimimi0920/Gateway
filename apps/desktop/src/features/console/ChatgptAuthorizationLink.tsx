import { useEffect, useState } from "react";
import { Button, ButtonLink, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { kiroAuthorizationUrl } from "./kiroAuthorizationUrl";
import { lobsteraiAuthorizationUrl } from "./lobsteraiAuthorizationUrl";
import { quickAuthorizationUrl } from "./quickAuthorizationUrl";
import { cursorAuthorizationUrl } from "./cursorAuthorizationUrl";

type AuthorizationKind = "cursor" | "chatgpt" | "antigravity" | "vertex" | "grok" | "grok-device" | "accio" | "kiro" | "quick" | "freebuff" | "dim-china" | "dim-global" | "nous" | "lobsterai" | "luma";
function authorizationUrl(value: string, kind: AuthorizationKind): string | null {
  if (kind === "cursor") return cursorAuthorizationUrl(value);
  if (kind === "lobsterai") return lobsteraiAuthorizationUrl(value);
  if (kind === "kiro") return kiroAuthorizationUrl(value);
  if (kind === "quick") return quickAuthorizationUrl(value);
  if (value.length > 8192) return null;
  try {
    const url = new URL(value);
    // Never turn an arbitrary server-supplied URL into an executable link.
    const google = kind === "antigravity" || kind === "vertex";
    const origin = kind === "dim-china" ? "https://dimagent.cn" : kind === "dim-global" ? "https://dimagent.com" : kind === "grok" ? "https://auth.x.ai" : google ? "https://accounts.google.com" : "https://auth.openai.com";
    const path = kind === "grok" ? "/oauth2/authorize" : google ? "/o/oauth2/v2/auth" : "/oauth/authorize";
    const allowed = kind === "luma" ? url.origin === "https://agents.lumalabs.ai" && url.pathname === "/mcp/oauth/authorize"
      : kind === "grok-device" ? ["https://auth.x.ai", "https://accounts.x.ai"].includes(url.origin) : kind === "accio"
      ? ["https://www.accio-ai.com", "https://zh.accio.com"].includes(url.origin) && url.pathname === "/login"
      : kind === "nous" ? url.origin === "https://portal.nousresearch.com" && value.length <= 4096 && !/[\x00-\x20]/.test(value)
      : kind === "freebuff" ? url.origin === "https://freebuff.com" : url.origin === origin && url.pathname === path;
    if (url.href.length > 8192 || !allowed
      || url.username || url.password || url.href.includes("#")) return null;
    return url.href;
  } catch { return null; }
}

export function ChatgptAuthorizationLink({ value, busy, kind = "chatgpt" }: { value: string; busy: boolean; kind?: AuthorizationKind }) {
  const { t } = useUiLocale();
  const [failed, setFailed] = useState(false);
  const href = authorizationUrl(value, kind);
  useEffect(() => { setFailed(false); }, [value, kind]);
  useEffect(() => {
    const fail = () => setFailed(true);
    window.addEventListener("gateway:oauth-browser-failed", fail);
    return () => window.removeEventListener("gateway:oauth-browser-failed", fail);
  }, []);
  const label = t("打开授权页面", "Open authorization page");
  return <>
    {href && !busy
      ? <ButtonLink href={href} target="_blank" rel="noopener noreferrer"
        onClick={() => setFailed(false)}>{label}</ButtonLink>
      : <Button disabled>{label}</Button>}
    {!href && <Notice tone="danger">{t("授权链接无效，请重新登录。", "Invalid authorization URL. Sign in again.")}</Notice>}
    {failed && <Notice tone="danger">{t("无法打开浏览器，请复制授权链接后手动打开。", "Cannot open the browser. Copy the authorization link and open it manually.")}</Notice>}
    {href && kind === "cursor" && <>
      <p className="n-muted">{t("若未打开浏览器，可复制下方链接到浏览器地址栏；使用同一链接，无需重复创建授权。", "If no browser opens, copy this same link into its address bar. Do not create another authorization.")}</p>
      <input aria-label="Cursor 授权链接" value={href} readOnly onFocus={event => event.target.select()} />
    </>}
  </>;
}
