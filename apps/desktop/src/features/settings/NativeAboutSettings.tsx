import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Field, Input, Notice, SegmentedControl } from "@neuro/ui";
import { z } from "zod";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { SettingRow } from "./SettingsControls";
import { projectUrl, releasesUrl } from "./aboutRelease";

const preferencesSchema = z.object({ showButton: z.boolean(), automatic: z.boolean(), intervalHours: z.union([z.literal(1), z.literal(6), z.literal(24)]),
  channel: z.enum(["stable", "all"]), autoDownload: z.boolean(), autoRestart: z.boolean(), downloadSource: z.string() });
const statusSchema = z.object({ preferences: preferencesSchema, currentVersion: z.string(), canInstall: z.boolean(), configDirectory: z.string(), busy: z.boolean(),
  release: z.object({ version: z.string(), notes: z.string(), page: z.string(), asset: z.string(), checksum: z.string() }).nullable(),
  downloaded: z.boolean(), lastChecked: z.number(), error: z.string().nullable() });
type Preferences = z.infer<typeof preferencesSchema>;
type Status = z.infer<typeof statusSchema>;

/** Runs exclusively in packaged local content. Remote console pages cannot invoke this updater. */
export function NativeAboutSettings({ onInteractionChange }: { onInteractionChange?: (busy: boolean) => void }) {
  const { t } = useUiLocale();
  const [status, setStatus] = useState<Status | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState("");
  const [dialog, setDialog] = useState<"" | "source" | "notes" | "install">("");
  const [source, setSource] = useState("");
  const alive = useRef(false); const running = useRef(false);
  const call = async (action: string, preferences?: Preferences) => statusSchema.parse(await invoke("gateway_about_action", { action, preferences: preferences ?? null }));
  useEffect(() => {
    alive.current = true;
    let reading = false;
    const poll = async () => {
      if (reading || running.current) return;
      reading = true;
      try { const value = await call("status"); if (alive.current) setStatus(value); }
      catch (error) { if (alive.current) setError(String(error)); }
      finally { reading = false; }
    };
    void poll(); const timer = setInterval(() => void poll(), 4000);
    return () => { alive.current = false; clearInterval(timer); };
  }, []);
  useEffect(() => { onInteractionChange?.(!!dialog); return () => onInteractionChange?.(false); }, [dialog, onInteractionChange]);
  const run = async (action: string, preferences?: Preferences) => {
    if (running.current) return;
    running.current = true; setPending(true); setError("");
    try { const value = await call(action, preferences); if (alive.current) { setStatus(value); setDialog(""); } }
    catch (error) { if (alive.current) setError(String(error)); }
    finally { running.current = false; if (alive.current) setPending(false); }
  };
  const prefs = status?.preferences;
  const busy = pending || status?.busy || !prefs;
  const save = (patch: Partial<Preferences>) => { if (prefs) void run("save", { ...prefs, ...patch }); };
  const toggle = (id: string, label: string, value: boolean | undefined, update: (value: boolean) => void, disabled = false, note?: string) => <SettingRow id={id} label={label} note={note}>
    <SegmentedControl hideLabel name={id} label={label} value={value ? "on" : "off"} disabled={busy || disabled}
      options={[{ value: "off", label: t("关闭", "Off") }, { value: "on", label: t("开启", "On") }]} onValueChange={value => update(value === "on")} />
  </SettingRow>;
  return <>
    <SettingRow id="version" label={t("版本", "Version")}><code>{status?.currentVersion ?? "—"}</code>
      {prefs?.showButton && <Button compact disabled={busy} onClick={() => void run("check")}>{t("检查更新", "Check for updates")}</Button>}
      {status?.release && !status.downloaded && <Button compact disabled={busy} onClick={() => void run("download")}>{t("下载更新", "Download update")} {status.release.version}</Button>}
      {status?.downloaded && <Button compact disabled={busy || !status.canInstall} onClick={() => setDialog("install")}>{t("重启并更新", "Restart and update")}</Button>}
    </SettingRow>
    <SettingRow id="release-notes" label={t("更新内容", "Release notes")}>{status?.release
      ? <Button compact onClick={() => setDialog("notes")}>{t("查看", "View")}</Button>
      : <a href={releasesUrl} onClick={event => { event.preventDefault(); void run("openReleases"); }}>{t("查看", "View")} ↗</a>}</SettingRow>
    {toggle("update-button", t("更新按钮", "Update button"), prefs?.showButton, showButton => save({ showButton }))}
    {toggle("auto-update", t("自动检查更新", "Automatic update checks"), prefs?.automatic, automatic => save(automatic ? { automatic } : { automatic, autoDownload: false, autoRestart: false }))}
    <SettingRow id="update-interval" label={t("检查间隔", "Check interval")}><SegmentedControl hideLabel name="update-interval" label={t("检查间隔", "Check interval")}
      value={String(prefs?.intervalHours ?? 24)} disabled={busy || !prefs?.automatic} options={[1, 6, 24].map(hours => ({ value: String(hours), label: t(`${hours} 小时`, `${hours} hours`) }))}
      onValueChange={hours => save({ intervalHours: Number(hours) as 1 | 6 | 24 })} /></SettingRow>
    <SettingRow id="update-channel" label={t("更新渠道", "Update channel")}><SegmentedControl hideLabel name="update-channel" label={t("更新渠道", "Update channel")}
      value={prefs?.channel ?? "stable"} disabled={busy} options={[{ value: "all", label: t("每个版本", "All releases") }, { value: "stable", label: t("稳定版本", "Stable") }]}
      onValueChange={channel => save({ channel: channel as Preferences["channel"] })} /></SettingRow>
    {toggle("update-download", t("自动下载更新", "Automatic download"), prefs?.autoDownload, autoDownload => save(autoDownload ? { autoDownload } : { autoDownload, autoRestart: false }), !prefs?.automatic)}
    {toggle("update-restart", t("自动重启更新", "Automatic restart"), prefs?.autoRestart, autoRestart => save({ autoRestart }), !status?.canInstall || !prefs?.autoDownload,
      status && !status.canInstall ? t("开发版和手测包不自动安装正式版本", "Development and hand-test builds do not install official releases automatically") : undefined)}
    <SettingRow id="update-source" label={t("下载源", "Download source")}><span>{prefs?.downloadSource || "GitHub"}</span><Button compact disabled={busy} onClick={() => { setSource(prefs?.downloadSource ?? ""); setDialog("source"); }}>{t("设置…", "Configure…")}</Button></SettingRow>
    <SettingRow id="config-directory" label={t("配置目录", "Configuration directory")}><Button compact disabled={busy} onClick={() => void run("openDirectory")}>{t("打开", "Open")}</Button></SettingRow>
    <SettingRow id="website" label={t("项目主页", "Project website")}><a href={projectUrl} onClick={event => { event.preventDefault(); void run("openProject"); }}>GitHub ↗</a></SettingRow>
    <SettingRow id="license" label={t("开源许可", "License")}><a href={`${projectUrl}/blob/main/LICENSE`} onClick={event => { event.preventDefault(); void run("openLicense"); }}>MIT ↗</a></SettingRow>
    <SettingRow id="community" label={t("社区", "Community")}><a href={`${projectUrl}/issues`} onClick={event => { event.preventDefault(); void run("openCommunity"); }}>{t("问题反馈", "Issue tracker")} ↗</a></SettingRow>
    {status?.lastChecked !== 0 && status?.lastChecked && <Notice>{t("上次检查：", "Last checked: ")}{new Date(status.lastChecked * 1000).toLocaleString()}{!status.release && !status.error ? t(" · 没有可用的新版本", " · No newer release available") : ""}</Notice>}
    {(error || status?.error) && <Notice tone="danger">{error || status?.error}<Button compact disabled={pending} onClick={() => void run("status")}>{t("重试", "Retry")}</Button></Notice>}
    <Dialog open={!!dialog} busy={pending} onOpenChange={open => { if (!open) setDialog(""); }} title={dialog === "source" ? t("下载源", "Download source") : dialog === "install" ? t("重启并更新", "Restart and update") : t("更新内容", "Release notes")}>
      {dialog === "source" ? <><Field label={t("HTTPS 镜像前缀（留空使用 GitHub）", "HTTPS mirror prefix (empty for GitHub)")}><Input value={source} maxLength={2048} disabled={pending} onChange={event => setSource(event.target.value)} /></Field>
        <Notice>{t("镜像仅用于下载压缩包；校验值始终从官方仓库获取。", "Mirrors only download the archive; checksums always come from the official repository.")}</Notice>
        <Button compact disabled={pending} onClick={() => save({ downloadSource: source.trim() })}>{t("保存", "Save")}</Button></>
        : dialog === "install" ? <><Notice tone="warning">{t("将退出此桌面客户端并更新其托管的本机后端。保留数据与旧版本；远程服务器不受影响。有调用正在执行时会推迟更新。", "Restart this desktop and its managed local backend. Data and previous versions are retained; remote servers are unchanged. Active requests postpone the update.")}</Notice><Button compact disabled={pending} onClick={() => void run("install")}>{t("确认更新", "Confirm update")}</Button></>
        : <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{status?.release?.notes || t("此版本未提供更新说明。", "No release notes provided.")}</pre>}
      {error && <Notice tone="danger">{error}</Notice>}
    </Dialog>
  </>;
}
