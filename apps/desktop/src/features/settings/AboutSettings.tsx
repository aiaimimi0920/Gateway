import { version } from "../../../package.json";
import { useEffect, useRef, useState } from "react";
import { Button, Dialog, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { SettingRow } from "./SettingsControls";
import { UnavailableSettings } from "./UnavailableSettings";
import { checkPublicRelease, projectUrl, releasesUrl, type PublicRelease } from "./aboutRelease";

export function AboutSettings() {
  const { t } = useUiLocale();
  const [release, setRelease] = useState<PublicRelease | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [notes, setNotes] = useState(false);
  const request = useRef<AbortController | null>(null);
  useEffect(() => () => { request.current?.abort(); request.current = null; }, []);
  const check = async () => {
    if (request.current) return;
    const controller = new AbortController(); request.current = controller;
    const timeout = setTimeout(() => controller.abort(), 20_000);
    setBusy(true); setMessage("");
    try { const value = await checkPublicRelease(controller.signal); if (!controller.signal.aborted) {
      setRelease(value); setMessage(value ? t(`最新正式版本：${value.version}`, `Latest release: ${value.version}`) : t("尚无正式发布版本。", "No stable release is available."));
    } } catch (error) { if (request.current === controller) setMessage(error instanceof Error ? error.message : t("检查失败，请重试。", "Check failed. Retry.")); }
    finally { clearTimeout(timeout); if (request.current === controller) { request.current = null; setBusy(false); } }
  };
  return <>
    <SettingRow id="version" label={t("版本", "Version")} note={t("管理界面构建版本，不代表所连接后端版本", "Console build version, not the connected backend version")}>
      <code>{version}</code>
      <Button compact disabled={busy} onClick={() => void check()}>{busy ? t("检查中…", "Checking…") : t("检查更新", "Check for updates")}</Button>
    </SettingRow>
    <SettingRow id="release-notes" label={t("更新内容", "Release notes")}>
      {release ? <Button compact onClick={() => setNotes(true)}>{t("查看", "View")}</Button> : <a href={releasesUrl} target="_blank" rel="noreferrer noopener">{t("查看", "View")} ↗</a>}
    </SettingRow>
    {message && <Notice>{message}</Notice>}
    {release && <a href={release.url} target="_blank" rel="noreferrer noopener">{t("查看版本与下载", "View release and downloads")} ↗</a>}
    <UnavailableSettings ids={["update-button", "auto-update", "update-interval", "update-channel", "update-download", "update-restart", "update-source", "config-directory"]} reason={t("仅桌面客户端可用", "Desktop client only")} />
    <SettingRow id="website" label={t("项目主页", "Project website")}><a href={projectUrl} target="_blank" rel="noreferrer noopener">GitHub ↗</a></SettingRow>
    <SettingRow id="license" label={t("开源许可", "License")}><a href={`${projectUrl}/blob/main/LICENSE`} target="_blank" rel="noreferrer noopener">MIT ↗</a></SettingRow>
    <SettingRow id="community" label={t("社区", "Community")}>
      <a href={`${projectUrl}/issues`} target="_blank" rel="noreferrer noopener">{t("问题反馈", "Issue tracker")} ↗</a>
    </SettingRow>
    <Dialog open={notes} onOpenChange={setNotes} title={t("更新内容", "Release notes")}><pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{release?.notes || t("此版本未提供更新说明。", "No release notes provided.")}</pre></Dialog>
  </>;
}
