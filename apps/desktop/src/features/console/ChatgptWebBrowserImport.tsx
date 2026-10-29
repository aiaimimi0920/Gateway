import { useEffect, useRef, useState } from "react";
import { parseChatgptWebMaterial, type ChatgptWebMaterial } from "./chatgptWebMaterial";

type LoginPhase = "login_window" | "session_read" | "verification_required" | "verification_window";
export type WebBrowserCapture = (input: { providerId: string; operation: "login" | "cancel" | "status" | "verify"; requestId: string },
  signal: AbortSignal) => Promise<{ phase?: LoginPhase; material?: unknown; accountName?: string; catalog?: { code?: string; status?: number } }>;

/** The owned login window is separate from the user's daily browser and Codex OAuth. */
export function ChatgptWebBrowserImport({ capture, providerId, disabled, onMaterial, onClear }: {
  capture: WebBrowserCapture;
  providerId: string;
  disabled: boolean;
  onMaterial(material: ChatgptWebMaterial, accountName?: string): void;
  onClear(): void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [accountName, setAccountName] = useState("");
  const [phase, setPhase] = useState<LoginPhase>("login_window");
  const [continuing, setContinuing] = useState(false);
  const request = useRef<{ controller: AbortController; requestId: string; verificationSubmitted: boolean; cancel(): void; stopPolling(): void } | null>(null);
  const stop = () => {
    const pending = request.current; request.current = null;
    pending?.stopPolling(); pending?.controller.abort(); pending?.cancel(); setBusy(false);
  };
  useEffect(() => {
    setBusy(false); setAccountName(""); setError(null);
    return () => {
      const pending = request.current; request.current = null;
      pending?.stopPolling(); pending?.controller.abort(); pending?.cancel();
    };
  }, [disabled, capture, providerId]);
  const run = async () => {
    if (request.current || disabled) return;
    const controller = new AbortController();
    const requestId = crypto.randomUUID();
    let pollTimer: ReturnType<typeof setTimeout> | undefined;
    let pollController: AbortController | undefined;
    const current = { controller, requestId, verificationSubmitted: false, stopPolling: () => { clearTimeout(pollTimer); pollController?.abort(); }, cancel: () => {
      void capture({ providerId, operation: "cancel", requestId }, AbortSignal.timeout(10000)).catch(() => {});
    } };
    request.current = current;
    setBusy(true); setContinuing(false); setPhase("login_window"); setError(null); setAccountName(""); onClear();
    const poll = async () => {
      if (request.current !== current || controller.signal.aborted) return;
      pollController = new AbortController();
      try {
        const result = await capture({ providerId, operation: "status", requestId }, AbortSignal.any([pollController.signal, AbortSignal.timeout(5000)]));
        if (request.current === current && result.phase && !(current.verificationSubmitted && result.phase === "verification_required")) setPhase(result.phase);
      } catch { /* The main transaction owns errors; status never restarts login or requests secrets. */ }
      if (request.current === current && !controller.signal.aborted) pollTimer = setTimeout(() => void poll(), 750);
    };
    pollTimer = setTimeout(() => void poll(), 750);
    try {
      const result = await capture({ providerId, operation: "login", requestId }, controller.signal);
      if (controller.signal.aborted || request.current !== current) return;
      const material = parseChatgptWebMaterial(JSON.stringify(result.material));
      onMaterial(material, result.accountName);
      setAccountName(result.accountName || "ChatGPT Web");
      if (result.catalog?.code) setError(result.catalog.code === "catalog_verification_required"
        ? "账号已读取；模型目录被拒绝（HTTP 403），需在官网完成人工验证。"
        : `账号已读取；模型目录未取得${result.catalog.status ? `（HTTP ${result.catalog.status}）` : ""}，可稍后重试或手动填写模型。`);
    } catch (cause) {
      if (!controller.signal.aborted && request.current === current) setError(cause instanceof Error ? cause.message : "网页登录失败。");
    } finally { current.stopPolling(); if (request.current === current) { request.current = null; setBusy(false); } }
  };
  const verify = async () => {
    const current = request.current;
    if (!current || continuing || current.verificationSubmitted || phase !== "verification_required") return;
    current.verificationSubmitted = true;
    setContinuing(true); setError(null);
    try {
      await capture({ providerId, operation: "verify", requestId: current.requestId }, current.controller.signal);
      if (request.current === current) setPhase((currentPhase) => currentPhase === "verification_required" ? "verification_window" : currentPhase);
    } catch (cause) {
      current.verificationSubmitted = false;
      if (!current.controller.signal.aborted && request.current === current) setError(cause instanceof Error ? cause.message : "无法打开验证窗口。");
    } finally { if (request.current === current) setContinuing(false); }
  };
  return <section className="nt-stack" aria-label="ChatGPT 网页登录">
    <button className="nt-btn nt-btn--outline" type="button" disabled={disabled || busy} onClick={() => void run()}>网页登录</button>
    {busy && <>
      <span role="status">{phase === "verification_required" ? "会话接口要求官网验证，尚未读取账号。"
        : phase === "verification_window" ? "请在专用窗口完成官网验证后关闭窗口；请勿复制页面内容。"
          : phase === "session_read" ? "正在读取已登录账号…" : "请完成登录，看到 ChatGPT 首页后关闭该专用窗口…"}</span>
      {phase === "verification_required" && <button className="nt-btn nt-btn--outline" type="button" disabled={continuing} onClick={() => void verify()}>打开验证窗口</button>}
      <button className="nt-btn nt-btn--outline" type="button" onClick={stop}>取消登录</button>
    </>}
    {accountName && <span role="status">已读取账号：{accountName}</span>}
    {error && <span role="alert">{error}</span>}
  </section>;
}
