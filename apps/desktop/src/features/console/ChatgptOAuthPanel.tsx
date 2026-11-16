import { useEffect, useMemo, useRef, useState } from "react";
import { Button, CopyButton, Disclosure, Field, Input, Notice, Select, Stack, Toolbar } from "@neuro/ui";
import type { GatewayApiClient } from "../../api/client";
import { GatewayApiError } from "../../api/errors";
import { createChatgptAuthApi, type ChatgptAuthSession } from "../../api/console/chatgpt-auth";
import { createAntigravityAuthApi } from "../../api/console/antigravity-auth";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { CHATGPT_POOL_CATEGORIES } from "./chatgptPool";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";

type Props = {
  kind?: "chatgpt" | "antigravity";
  client: GatewayApiClient; managementToken: string | null; secretGrant: string | null;
  providerId: string; disabled: boolean; onRequestSecretAccess(): void | Promise<void>; onSaved(): void;
};

// Share only operator-session UI; OAuth transports and credential owners remain independent.
export function ChatgptOAuthPanel({ kind = "chatgpt", client, managementToken, secretGrant, providerId, disabled, onRequestSecretAccess, onSaved }: Props) {
  const { t } = useUiLocale();
  const api = useMemo(() => (kind === "antigravity" ? createAntigravityAuthApi : createChatgptAuthApi)(client, managementToken ?? ""), [client, managementToken, kind]);
  const [group, setGroup] = useState("free");
  const [session, setSession] = useState<ChatgptAuthSession | null>(null);
  const [callbackUrl, setCallbackUrl] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingAccess, setPendingAccess] = useState<{ action: "create" | "import"; previous: string | null; generation: number } | null>(null);
  const [saving, setSaving] = useState(false);
  const sessionRef = useRef<ChatgptAuthSession | null>(null);
  const generation = useRef(0);
  const update = (next: ChatgptAuthSession | null) => { sessionRef.current = next; setSession(next); };
  const handleError = (cause: unknown, fallback: string) => {
    if (cause instanceof GatewayApiError && ["chatgpt_auth_session_unavailable", "antigravity_auth_session_unavailable"].includes(cause.code ?? "")) {
      // Discard only server-confirmed missing sessions, never retryable import material.
      generation.current += 1;
      update(null); setCallbackUrl(""); setBusy(false); setPendingAccess(null); setSaving(false);
      setError(t("登录会话已过期或不可用，请重新登录。", "Login session expired or unavailable. Sign in again."));
      return true;
    }
    setError(cause instanceof Error ? cause.message : fallback);
    return false;
  };
  useEffect(() => {
    generation.current += 1;
    update(null); setError(null); setCallbackUrl(""); setBusy(false); setPendingAccess(null); setSaving(false);
    return () => {
      generation.current += 1;
      const previous = sessionRef.current;
      if (previous && previous.status !== "succeeded") void api.act(previous.id, "cancel").catch(() => { });
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

  const requestAccess = async (action: "create" | "import") => {
    const current = generation.current;
    setPendingAccess({ action, previous: secretGrant, generation: current }); setError(null);
    try { await onRequestSecretAccess(); }
    catch (cause) {
      if (generation.current === current) { setPendingAccess(null); handleError(cause, t("保存权限恢复失败，请重试。", "Cannot restore save access; retry.")); }
    }
  };
  const run = async (action: "create" | "cancel" | "complete" | "import", renewed = false) => {
    if ((action === "create" || action === "import") && !secretGrant) { await requestAccess(action); return; }
    setPendingAccess(null);
    const current = generation.current;
    setBusy(true); setSaving(action === "import"); setError(null);
    try {
      const result = action === "create"
        ? await api.create(providerId, group, secretGrant!)
        : await api.act(session!.id, action, secretGrant ?? undefined, action === "complete" ? callbackUrl : undefined);
      if (generation.current !== current) { if (action === "create") void api.act(result.session.id, "cancel").catch(() => { }); return; }
      update(result.session); setCallbackUrl("");
      if (result.session.status === "succeeded") onSaved();
    } catch (cause) {
      if (generation.current === current) {
        if (!renewed && (action === "create" || action === "import") && cause instanceof GatewayApiError && cause.code === "console_secret_access_required") await requestAccess(action);
        else handleError(cause, t("OAuth 登录失败", "OAuth login failed"));
      }
    } finally { if (generation.current === current) { setBusy(false); setSaving(false); } }
  };
  useEffect(() => {
    // Resume only the explicit user action, within the same provider/session generation.
    if (pendingAccess && pendingAccess.generation === generation.current && secretGrant && secretGrant !== pendingAccess.previous && !disabled && !busy) {
      const action = pendingAccess.action;
      setPendingAccess(null);
      if (action === "create" || sessionRef.current?.status === "ready") void run(action, true);
    }
  }, [pendingAccess, secretGrant, disabled, busy]);
  const active = session && !["succeeded", "cancelled", "failed"].includes(session.status);
  return <section className="n-stack" aria-label={kind === "antigravity" ? "Antigravity OAuth" : t("ChatGPT OAuth 登录", "ChatGPT OAuth sign-in")}>
    <Stack>
      {kind === "chatgpt" && <Field label={t("订阅分组", "Subscription group")}>
        <Select value={group} disabled={busy || Boolean(active) || Boolean(pendingAccess)} onChange={(event) => setGroup(event.target.value)}>
          {CHATGPT_POOL_CATEGORIES.map((category) => <option key={category.id} value={category.id}>{category.label}</option>)}
        </Select>
      </Field>}
      <Button variant={active ? "secondary" : "primary"} disabled={disabled || busy || Boolean(active) || Boolean(pendingAccess)} onClick={() => void run("create")}>
        {kind === "antigravity" ? t("通过 Antigravity 登录", "Sign in with Antigravity") : t("通过 ChatGPT 登录", "Sign in with ChatGPT")}
      </Button>
    </Stack>
    {disabled && <Notice role="note">{t("请等待当前配置保存完成后登录。", "Wait for the current configuration to finish saving.")}</Notice>}
    {pendingAccess && <Notice>{t("正在恢复保存权限，恢复后会继续保存，无需重新登录。", "Restoring save access; saving will resume without another login.")}</Notice>}
    {session && <Notice tone={session.status === "succeeded" ? "success" : "info"}>{saving ? t("正在保存到凭证池，请稍候…", "Saving to credential pool; please wait…") : session.message}</Notice>}
    {session?.status === "waiting_user" && <>
      <Toolbar>
        <ChatgptAuthorizationLink key={session.id} value={session.authorizationUrl} busy={busy} kind={kind} />
        <CopyButton label={t("复制授权链接", "Copy authorization link")} copiedLabel={t("已复制", "Copied")} errorLabel={t("复制失败", "Copy failed")}
          onCopy={async () => {
            try { await navigator.clipboard.writeText(session.authorizationUrl); }
            catch (cause) { setError(t("复制失败，请手动复制链接。", "Copy failed; copy the link manually.")); throw cause; }
          }} />
      </Toolbar>
      <Field label={t("授权链接", "Authorization URL")}><Input readOnly value={session.authorizationUrl} /></Field>
      <p className="n-muted">{t("授权页面在当前设备的浏览器打开。连接远程 Gateway 时，若回调页面无法访问，请复制地址栏中的完整回调地址并粘贴到下方。", "Authorization opens in this device's browser. With a remote Gateway, if the callback page is unreachable, copy its complete address from the address bar and paste it below.")}</p>
      <Disclosure title={t("未自动返回？粘贴回调地址", "No automatic callback? Paste the callback URL")}>
        <Stack>
          <Field label={t("回调地址", "Callback URL")}><Input value={callbackUrl} onChange={(event) => setCallbackUrl(event.target.value)} placeholder={kind === "antigravity" ? "http://localhost:51121/oauth-callback?..." : "http://localhost:1455/auth/callback?..."} /></Field>
          <Button disabled={busy || !callbackUrl} onClick={() => void run("complete")}>{t("完成授权", "Complete authorization")}</Button>
        </Stack>
      </Disclosure>
    </>}
    {session?.status === "ready" && <Button variant="primary" loading={saving} aria-label={saving ? t("正在保存…", "Saving…") : undefined} disabled={disabled || busy || Boolean(pendingAccess)} onClick={() => void run("import")}>{t("保存到凭证池", "Save to credential pool")}</Button>}
    {active && <Button disabled={busy} onClick={() => void run("cancel")}>{t("取消登录", "Cancel login")}</Button>}
    {error && <Notice tone="danger">{error}</Notice>}
  </section>;
}
