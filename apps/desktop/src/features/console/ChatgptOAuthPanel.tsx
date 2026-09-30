import { useEffect, useMemo, useRef, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import { createChatgptAuthApi, type ChatgptAuthSession } from "../../api/console/chatgpt-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";

type Props = {
  client: GatewayApiClient; managementToken: string | null; secretGrant: string | null;
  providerId: string; disabled: boolean; onRequestSecretAccess(): void; onSaved(): void;
};

export function ChatgptOAuthPanel({ client, managementToken, secretGrant, providerId, disabled, onRequestSecretAccess, onSaved }: Props) {
  const { t } = useUiLocale();
  const api = useMemo(() => createChatgptAuthApi(client, managementToken ?? ""), [client, managementToken]);
  const [group, setGroup] = useState("free");
  const [session, setSession] = useState<ChatgptAuthSession | null>(null);
  const [callbackUrl, setCallbackUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sessionRef = useRef<ChatgptAuthSession | null>(null);
  const generation = useRef(0);
  const update = (next: ChatgptAuthSession | null) => { sessionRef.current = next; setSession(next); };
  const handleError = (cause: unknown, fallback: string) => {
    if (cause instanceof GatewayApiError && cause.code === "chatgpt_auth_session_unavailable") {
      // Discard only server-confirmed missing sessions, never retryable import material.
      generation.current += 1;
      update(null); setCallbackUrl(""); setBusy(false);
      setError(t("登录会话已过期或不可用，请重新登录。", "Login session expired or unavailable. Sign in again."));
      return true;
    }
    setError(cause instanceof Error ? cause.message : fallback);
    return false;
  };
  useEffect(() => {
    generation.current += 1;
    update(null); setError(null); setCallbackUrl(""); setBusy(false);
    return () => {
      generation.current += 1;
      const previous = sessionRef.current;
      if (previous && previous.status !== "succeeded") void api.act(previous.id, "cancel").catch(() => {});
    };
  }, [api, providerId]);
  useEffect(() => {
    if (busy || !session || !["waiting_user", "exchanging", "ready", "importing"].includes(session.status)) return;
    const abort = new AbortController();
    const current = generation.current;
    let timer: ReturnType<typeof setTimeout>;
    let failures = 0;
    const poll = async () => {
      try {
        const result = await api.get(session.id, abort.signal);
        if (generation.current !== current || abort.signal.aborted) return;
        failures = 0; update(result.session);
        // The import may have committed even when its HTTP response was lost.
        if (result.session.status === "succeeded") { setError(null); onSaved(); return; }
      } catch (cause) {
        if (!abort.signal.aborted && generation.current === current
          && handleError(cause, t("登录状态读取失败", "Cannot read login status"))) return;
        failures += 1;
      }
      if (!abort.signal.aborted && failures < 5) timer = setTimeout(poll, 1500);
    };
    timer = setTimeout(poll, 1500);
    return () => { abort.abort(); clearTimeout(timer); };
  }, [api, session?.id, session?.status, busy, t, onSaved]);

  const run = async (action: "create" | "cancel" | "complete" | "import" | "open-browser") => {
    if ((action === "create" || action === "import") && !secretGrant) { onRequestSecretAccess(); return; }
    const current = generation.current;
    setBusy(true); setError(null);
    try {
      const result = action === "create"
        ? await api.create(providerId, group, secretGrant!)
        : await api.act(session!.id, action, secretGrant ?? undefined, action === "complete" ? callbackUrl : undefined);
      if (generation.current !== current) { if (action === "create") void api.act(result.session.id, "cancel").catch(() => {}); return; }
      update(result.session); setCallbackUrl("");
      if (result.session.status === "succeeded") onSaved();
    } catch (cause) {
      if (generation.current === current) handleError(cause, t("ChatGPT 登录失败", "ChatGPT login failed"));
    } finally { if (generation.current === current) setBusy(false); }
  };
  const active = session && !["succeeded", "cancelled", "failed"].includes(session.status);
  return <section className="nt-stack" aria-label={t("ChatGPT OAuth 登录", "ChatGPT OAuth sign-in")}>
    <div className="nt-grid nt-grid--2">
      <label className="nt-field"><span>{t("订阅分组", "Subscription group")}</span>
        <select className="nt-select" value={group} disabled={busy || Boolean(active)} onChange={(event) => setGroup(event.target.value)}>
          {CHATGPT_POOL_CATEGORIES.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}
        </select>
      </label>
      <button type="button" className="nt-btn nt-btn--primary" disabled={disabled || busy || Boolean(active)} onClick={() => void run("create")}>
        {t("通过 ChatGPT 登录", "Sign in with ChatGPT")}
      </button>
    </div>
    {disabled && <p>{t("请等待当前配置保存完成后登录。", "Wait for the current configuration to finish saving.")}</p>}
    {session && <p role="status">{session.message}</p>}
    {session?.status === "waiting_user" && <>
      <div className="nt-actions">
        <button type="button" className="nt-btn nt-btn--secondary" disabled={busy} onClick={() => void run("open-browser")}>{t("打开授权页面", "Open authorization page")}</button>
        <button type="button" className="nt-btn nt-btn--secondary" onClick={() => void Promise.resolve().then(() => navigator.clipboard.writeText(session.authorizationUrl)).catch(() => setError(t("复制失败，请手动复制链接。", "Copy failed; copy the link manually.")))}>{t("复制授权链接", "Copy authorization link")}</button>
      </div>
      <input className="nt-input" readOnly value={session.authorizationUrl} aria-label={t("授权链接", "Authorization URL")} />
      <details><summary>{t("未自动返回？粘贴回调地址", "No automatic callback? Paste the callback URL")}</summary>
        <input className="nt-input" value={callbackUrl} onChange={(event) => setCallbackUrl(event.target.value)} placeholder="http://localhost:1455/auth/callback?..." aria-label={t("回调地址", "Callback URL")} />
        <button type="button" className="nt-btn nt-btn--secondary" disabled={busy || !callbackUrl} onClick={() => void run("complete")}>{t("完成授权", "Complete authorization")}</button>
      </details>
    </>}
    {session?.status === "ready" && <button type="button" className="nt-btn nt-btn--primary" disabled={disabled || busy} onClick={() => void run("import")}>{t("保存到凭证池", "Save to credential pool")}</button>}
    {active && <button type="button" className="nt-btn nt-btn--secondary" disabled={busy} onClick={() => void run("cancel")}>{t("取消登录", "Cancel login")}</button>}
    {error && <p role="alert">{error}</p>}
  </section>;
}
