import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useId } from "react";
import { Notice, SegmentedControl } from "@neuro/ui";
import type { useDesktopPreferences } from "./useDesktopPreferences";
import { SettingRow } from "./SettingsControls";

export function DesktopBehaviorSettings({ preferences }: { preferences: ReturnType<typeof useDesktopPreferences> }) {
  const { t } = useUiLocale();
  const id = useId();
  const { value, pending, failed, update } = preferences;
  return <>
    <SettingRow id="awake" label={t("防止休眠", "Prevent sleep")}>
      <SegmentedControl<"off" | "system" | "display"> hideLabel name={`${id}-awake`} label={t("防止休眠", "Prevent sleep")} value={value?.sleepMode ?? "off"} disabled={!value?.sleepSupported || pending}
        onValueChange={sleepMode => void update({ sleepMode })}
        options={[{ value: "off", label: t("关闭", "Off") }, { value: "system", label: t("开启", "On") }, { value: "display", label: t("屏幕也常亮", "Keep screen on") }]} />
    </SettingRow>
    <SettingRow id="close" label={t("关闭窗口时", "On close")}>
      <SegmentedControl<"exit" | "tray"> hideLabel name={`${id}-close`} label={t("关闭窗口时", "On close")} value={value?.closeBehavior ?? "exit"} disabled={!value || pending}
        onValueChange={closeBehavior => void update({ closeBehavior })}
        options={[{ value: "exit", label: t("退出程序", "Exit application") }, { value: "tray", label: t("最小化到托盘", "Minimize to tray") }]} />
    </SettingRow>
    <SettingRow id="startup" label={t("开机启动", "Auto-start")}>
      <SegmentedControl hideLabel name={`${id}-startup`} label={t("开机启动", "Auto-start")} value={value?.autostart ? "on" : "off"} disabled={!value?.autostartSupported || pending}
        onValueChange={next => void update({ autostart: next === "on" })}
        options={[{ value: "off", label: t("关闭", "Off") }, { value: "on", label: t("开启", "On") }]} />
    </SettingRow>
    {(failed || pending) && <Notice tone={failed ? "danger" : "info"}>{failed ? t("未能确认桌面设置，请检查当前配置后重试。", "Could not confirm desktop settings. Check the current configuration and retry.") : t("保存中…", "Saving…")}</Notice>}
  </>;
}
