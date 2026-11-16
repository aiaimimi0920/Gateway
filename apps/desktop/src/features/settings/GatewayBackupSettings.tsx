import { useEffect, useMemo, useRef, useState } from "react";
import { Button, Dialog, Field, Input, Notice, SegmentedControl } from "@neuro/ui";
import { useGatewayHost } from "../../platform/HostProvider";
import { useManagementSession } from "../../session/useManagementSession";
import { SettingRow } from "./SettingsControls";
import { BackupDestinationForm, emptyDestination } from "./BackupDestinationForm";
import { gatewayBackupApi, readBackupFile, saveBackupFile, type BackupDestination, type BackupPreview } from "./gatewayBackupApi";

type Mode = "" | "settings" | "export" | "import" | "upload";
/** All data operations belong to the selected backend; browser storage never holds backup passwords. */
export function GatewayBackupSettings({ onInteractionChange }: { onInteractionChange?: (locked: boolean) => void }) {
  const host = useGatewayHost(); const session = useManagementSession();
  const api = useMemo(() => gatewayBackupApi(host), [host]);
  const [destination, setDestination] = useState<BackupDestination | null>(null);
  const [draft, setDraft] = useState(emptyDestination("webdav"));
  const [mode, setMode] = useState<Mode>(""); const [password, setPassword] = useState(""); const [repeat, setRepeat] = useState("");
  const [content, setContent] = useState(""); const [preview, setPreview] = useState<BackupPreview | null>(null);
  const [revision, setRevision] = useState(""); const [remoteRevision, setRemoteRevision] = useState<string | null>(null);
  const [busy, setBusy] = useState(false); const [ready, setReady] = useState(false); const [pending, setPending] = useState(false);
  const [error, setError] = useState(""); const [notice, setNotice] = useState("");
  const input = useRef<HTMLInputElement>(null); const request = useRef<AbortController | null>(null); const epoch = useRef(0);
  const token = session.managementToken; const grant = session.secretGrant?.grant;
  useEffect(() => { onInteractionChange?.(busy || !!mode); }, [busy, mode, onInteractionChange]);
  useEffect(() => () => onInteractionChange?.(false), [onInteractionChange]);
  useEffect(() => {
    const version = ++epoch.current; const controller = new AbortController(); request.current = controller;
    setReady(false); setBusy(false); setMode(""); setPassword(""); setRepeat(""); setContent(""); setPreview(null);
    setRemoteRevision(null); setDestination(null); setPending(false); setError(""); setNotice(""); setDraft(emptyDestination("webdav"));
    if (token && grant) void api({ action: "status" }, token, grant, controller.signal).then(value => {
      if (version !== epoch.current) return;
      setDestination(value.destination ?? null); setDraft(value.destination ?? emptyDestination("webdav"));
      setPending(value.pending === true); setRevision(value.revision ?? ""); setReady(true);
    }).catch(error => { if (version === epoch.current && !controller.signal.aborted) setError(`无法读取备份设置：${error instanceof Error ? error.message : "请确认网关后端已更新且管理权限有效。"}`); })
      .finally(() => { if (version === epoch.current) request.current = null; });
    else request.current = null;
    return () => { epoch.current++; controller.abort(); request.current?.abort(); };
  }, [api, token, grant]);
  const close = () => { if (!busy) { setMode(""); setPassword(""); setRepeat(""); setContent(""); setPreview(null); setError(""); setDraft(destination ?? emptyDestination("webdav")); } };
  const run = async (action: Record<string, unknown>, accept: (value: Awaited<ReturnType<typeof api>>) => void) => {
    if (!token || !grant || request.current) return;
    const version = epoch.current; const controller = new AbortController(); request.current = controller; setBusy(true); setError(""); setNotice("");
    try { const value = await api(action, token, grant, controller.signal); if (version === epoch.current) accept(value); }
    catch (error) { if (version === epoch.current) setError(`操作未完成：${error instanceof Error ? error.message : "请检查设置后重试。"}`); }
    finally { if (version === epoch.current) { request.current = null; setBusy(false); } }
  };
  const disabled = !ready || busy || pending;
  const passwordValid = password.length >= 8 && password.length <= 1024;
  const finish = (message: string) => { setMode(""); setPassword(""); setRepeat(""); setContent(""); setPreview(null); setNotice(message); };
  const chooseFile = async (file?: File) => {
    if (!file) return;
    const version = epoch.current;
    try { const data = await readBackupFile(file); if (version === epoch.current) { setContent(data); setPreview(null); setPassword(""); setMode("import"); setError(""); } }
    catch (error) { if (version === epoch.current) setError(error instanceof Error ? error.message : "无法读取备份文件。"); }
  };
  return <>
    <SettingRow id="sync" label="同步">
      <SegmentedControl hideLabel name="backup-destination" label="同步方式" value={destination?.kind ?? ""} disabled={disabled}
        options={[{ value: "webdav", label: "WebDAV" }, { value: "s3", label: "S3" }, { value: "github", label: "GitHub" }]}
        onValueChange={kind => { setDraft(destination?.kind === kind ? destination : emptyDestination(kind as BackupDestination["kind"])); setMode("settings"); }} />
      <Button compact disabled={disabled} onClick={() => { setDraft(destination ?? emptyDestination("webdav")); setMode("settings"); }}>设置…</Button>
      <Button compact disabled={disabled || !destination} onClick={() => { setMode("upload"); setPassword(""); setRepeat(""); }}>上传备份</Button>
      <Button compact disabled={disabled || !destination} onClick={() => void run({ action: "download" }, value => {
        if (!value.content || !value.remoteRevision) throw new Error("远端未返回完整备份。");
        setContent(value.content); setRemoteRevision(value.remoteRevision); setPreview(null); setPassword(""); setMode("import");
      })}>下载备份</Button>
    </SettingRow>
    <SettingRow id="backup-export" label="导出"><Button compact disabled={disabled} onClick={() => { setMode("export"); setPassword(""); setRepeat(""); }}>导出…</Button></SettingRow>
    <SettingRow id="backup-import" label="导入"><Button compact disabled={disabled} onClick={() => input.current?.click()}>导入…</Button>
      <input ref={input} hidden type="file" aria-label="选择加密备份" accept=".ngbk" onChange={event => { void chooseFile(event.target.files?.[0]); event.target.value = ""; }} />
    </SettingRow>
    {pending && <Notice>恢复已准备好。请完全退出使用此数据目录的 Gateway 桌面端和后端，再重新启动。启动时将覆盖配置、账号、访问密钥及余额，并保留恢复前副本。
      <Button compact disabled={busy} onClick={() => void run({ action: "cancelRestore" }, value => {
        if (value.pending !== false) throw new Error("取消状态未确认。"); setPending(false); finish("已取消待恢复任务，当前数据未改变。");
      })}>取消待恢复</Button></Notice>}
    {!grant && <Notice>正在获取敏感操作权限，请稍候。</Notice>}
    {notice && <Notice>{notice}</Notice>}
    {error && !mode && <Notice tone="danger">{error}</Notice>}
    <Dialog open={!!mode} busy={busy} onOpenChange={open => { if (!open) close(); }} title={mode === "settings" ? "同步设置" : mode === "import" ? "恢复备份" : mode === "upload" ? "上传备份" : "导出备份"}>
      {mode === "settings" ? <>
        <BackupDestinationForm value={draft} disabled={busy} onChange={setDraft} />
        <Button compact disabled={busy} onClick={() => void run({ action: "saveDestination", destination: draft }, value => {
          if (!value.saved) throw new Error("保存结果未确认。"); setDestination(draft); setRemoteRevision(null); finish("同步设置已保存。");
        })}>保存</Button>
      </> : <>
        <Field label="备份密码"><Input compact type="password" aria-label="备份密码" autoComplete="new-password" maxLength={1024} disabled={busy} value={password}
          onChange={event => { setPassword(event.target.value); setPreview(null); }} /></Field>
        {mode !== "import" && <Field label="确认密码"><Input compact type="password" aria-label="确认密码" autoComplete="new-password" maxLength={1024} disabled={busy} value={repeat} onChange={event => setRepeat(event.target.value)} /></Field>}
        {mode === "import" ? <>
          {!preview && <Button compact disabled={busy || !passwordValid || !content} onClick={() => void run({ action: "preview", content, password }, value => {
            if (!value.preview || !value.revision) throw new Error("备份校验未完成。"); setPreview(value.preview); setRevision(value.revision);
          })}>校验备份</Button>}
          {preview && <><p>{preview.createdAt} · {preview.providers} 个服务商 · {preview.accounts} 个账号</p>
            <Notice tone="warning">将替换此网关的配置、账号、管理密钥、AI 调用密钥和余额。重启前新增的数据也会被替换；恢复前的旧数据会保留为副本。</Notice>
            <Button compact disabled={busy} onClick={() => void run({ action: "restore", content, password, expectedRevision: revision, confirmed: true }, value => {
              if (!value.pending) throw new Error("恢复状态未确认。"); setPending(true); finish("备份已校验并准备恢复。");
            })}>确认覆盖并准备恢复</Button></>}
          <Button compact disabled={busy || !content} onClick={() => saveBackupFile(content)}>保存加密文件</Button>
        </> : <>
          {mode === "upload" && remoteRevision && <Notice tone="warning">将覆盖上次下载的远端备份；远端版本已变化时会拒绝覆盖。</Notice>}
          <Button compact disabled={busy || !passwordValid || password !== repeat} onClick={() => void run(mode === "upload"
            ? { action: "upload", password, expectedRemoteRevision: remoteRevision }
            : { action: "export", password }, value => {
              if (mode === "export") { if (!value.content) throw new Error("未收到备份文件。"); saveBackupFile(value.content); }
              else { if (!value.uploaded) throw new Error("上传结果未确认。"); setRemoteRevision(null); }
              finish(mode === "upload" ? "加密备份已上传。" : "加密备份已导出。");
            })}>{mode === "upload" ? "上传加密备份" : "导出加密备份"}</Button>
        </>}
      </>}
      {error && <Notice tone="danger">{error}</Notice>}
    </Dialog>
  </>;
}
