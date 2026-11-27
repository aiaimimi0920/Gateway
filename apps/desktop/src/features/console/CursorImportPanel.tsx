import { useEffect, useState } from "react";
import { Button, Field, Input, Notice } from "@neuro/ui";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { useCursorImport, type CursorImportProps } from "./useCursorImport";

export function CursorImportPanel(props: CursorImportProps) {
  const flow = useCursorImport(props);
  const [token, setToken] = useState("");
  const [manual, setManual] = useState(false);
  useEffect(() => {
    setToken(""); setManual(false);
  }, [props.providerId, props.credentialId]);
  const session = flow.session;
  const locked = flow.busy || flow.pending || session?.status === "save_unconfirmed";
  useEffect(() => { props.onActivityChange?.(locked); return () => props.onActivityChange?.(false); }, [locked, props.onActivityChange]);
  const messages: Record<string, string> = {
    waiting_user: "授权链接已生成。请点击下方“打开授权页面”，在 Cursor 网页批准本次授权；批准后自动接收、核验并保存，无需再回来点击确认。",
    verifying: "认证材料已收到，正在核验授权账号身份。",
    awaiting_permission: "令牌身份已核验，但 Gateway 敏感操作授权已到期。请续期后继续保存，无需重新登录 Cursor；材料仅在本次十分钟会话内保留。",
    importing: "令牌身份已核验，正在持久保存；请勿重复提交。",
    succeeded: "凭据已保存并读回确认。本次导入未发送推理请求；账号是否启用，请查看账号列表中的调度开关。",
    failed: "导入未成功；已收到认证材料与否见下方，不能据此判断浏览器已退出登录。",
    cancelled: "本次导入已取消，原有账号保留。", expired: "本次授权等待已到期，不代表浏览器未登录。",
    save_unconfirmed: "保存结果未确认，请核对账号列表与事务恢复结果，不要重复提交。",
  };
  return <section className="n-stack" aria-label="Cursor 账号导入">
    <Notice>此处仅保存本人授权的 Cursor 登录凭据，不是 Cloud Agents API Key。导入不会启动本地 Cursor、发送推理请求或启用付费回落。</Notice>
    {props.savedAccount && <Notice>{props.savedAccount.hasToken
      ? `已保存凭据：${props.savedAccount.name}。${props.savedAccount.expiresAt && Number.isFinite(Date.parse(props.savedAccount.expiresAt))
        ? Date.parse(props.savedAccount.expiresAt) <= Date.now() ? "令牌已过期，需要重新授权。" : "令牌尚在标注有效期内；未重新核验上游有效性。"
        : "令牌到期时间未知。"}`
      : "此账号缺少已保存的访问令牌，请通过下方流程导入。"}</Notice>}
    {manual && <Notice>只粘贴 access token 原文（三段 JWT），不要粘贴 Bearer 前缀、Cookie、密码或整个 JSON；也不要发送到聊天中。</Notice>}
    <p className="n-muted">{manual ? "无需填写邮箱，校验令牌后自动识别并保存账号。"
      : "无需填写邮箱，授权后自动识别并保存账号。请在 Cursor 网页选择要导入的本人账号。"}</p>
    {props.savedAccount?.hasToken && <p className="n-muted">重新授权必须使用原账号；不同账号不会覆盖已保存凭据。</p>}
    <label><input type="checkbox" checked={manual} disabled={locked} onChange={event => { setManual(event.target.checked); setToken(""); }} />手动导入已有访问令牌</label>
    {!manual && !flow.pending && <p className="n-muted">先生成授权链接，再点击“打开授权页面”。此按钮不会自动弹出浏览器；网页批准后才会自动导入。</p>}
    {manual && <Field label="Cursor access token"><Input type="password" value={token} maxLength={32768} autoComplete="off" spellCheck={false}
      disabled={locked} onChange={event => setToken(event.target.value)} /></Field>}
    <Button variant="primary" loading={flow.busy} disabled={locked || props.disabled || !props.expectedRevision || manual && !token.trim()}
      onClick={() => {
        if (!props.secretGrant) { void props.onRequestSecretAccess(); return; }
        const material = manual ? token.trim() : undefined; setToken("");
        void flow.start(material);
      }}>{manual ? "校验并导入" : session?.status === "waiting_user" ? "等待网页确认" : "生成 Cursor 授权链接"}</Button>
    {!props.expectedRevision || props.disabled ? <Notice>请先保存服务商及当前配置。</Notice> : null}
    {session && <Notice tone={session.status === "succeeded" ? "success" : session.errorCode ? "warning" : "info"}>
      {messages[session.status]}{session.errorCode && <code> {session.errorCode}</code>}
      {session.errorCode === "cursor_import_identity_conflict" && <p>授权账号与原账号不同，或该账号已存在于此服务商。原有凭据未被覆盖，请核对账号列表与网页所选账号。</p>}
      <p>材料已收到：{session.authReceived ? "是" : "否"} · 令牌身份已核验：{session.accountMatched ? "是" : "未确认"} · 已保存：{session.credentialPersisted ? "是" : "未确认"}</p>
    </Notice>}
    {session?.authorizationUrl && <ChatgptAuthorizationLink value={session.authorizationUrl} kind="cursor" busy={flow.busy} />}
    {session?.status === "awaiting_permission" && <Button disabled={flow.busy} onClick={() => void flow.renewPermission()}>确认权限并继续保存</Button>}
    {session && ["waiting_user", "verifying", "awaiting_permission"].includes(session.status) && <Button disabled={flow.busy} onClick={() => void flow.cancel()}>取消本次导入</Button>}
    {flow.pending && <p className="n-muted">关闭此面板不会取消已接纳的导入。需要停止时请点击取消。</p>}
    {flow.paused && <Button onClick={flow.resume}>继续检查状态</Button>}
    {(flow.paused && flow.unavailable || session?.status === "save_unconfirmed") && <Notice tone="warning">导入结果需要人工核对。请先刷新账号列表并确认事务恢复已完成；已有账号应使用“重新授权此账号”，不要重复新增。
      <Button onClick={flow.releaseUnknown}>我已核对账号列表，解除旧会话</Button></Notice>}
    {flow.error && <Notice tone="warning">{flow.error}</Notice>}
  </section>;
}
