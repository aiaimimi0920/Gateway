import { useId } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { useDesktopPreferences } from "./useDesktopPreferences";

export function DesktopBehaviorSettings({ preferences }: { preferences: ReturnType<typeof useDesktopPreferences> }) {
  const { t } = useUiLocale();
  const id = useId();
  const { value, pending, failed, update } = preferences;
  return <>
    <div className="nt-settings-fields">
      <div className="nt-setting-row"><label htmlFor={`${id}-close`}>{t("关闭窗口时", "On close")}</label>
        <select className="nt-input" id={`${id}-close`} value={value?.closeBehavior ?? "exit"} disabled={!value || pending}
          onChange={event => void update({ closeBehavior: event.target.value === "tray" ? "tray" : "exit" })}>
          <option value="tray">{t("最小化到托盘", "Minimize to tray")}</option><option value="exit">{t("退出程序", "Exit application")}</option>
        </select></div>
      <div className="nt-setting-row"><label htmlFor={`${id}-startup`}>{t("开机启动", "Auto-start")}</label>
        <select className="nt-input" id={`${id}-startup`} value={value?.autostart ? "on" : "off"} disabled={!value?.autostartSupported || pending}
          onChange={event => void update({ autostart: event.target.value === "on" })}>
          <option value="on">{t("开启", "On")}</option><option value="off">{t("关闭", "Off")}</option>
        </select></div>
    </div>
    <p className="nt-settings-feedback" role={failed ? "alert" : "status"}><span>{failed ? t("未能确认桌面设置，请检查当前配置后重试。", "Could not confirm desktop settings. Check the current configuration and retry.") : pending ? t("保存中…", "Saving…") : ""}</span></p>
  </>;
}
