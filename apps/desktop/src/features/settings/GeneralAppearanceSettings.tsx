import { useId, useState } from "react";
import { Notice, SegmentedControl, Select } from "@neuro/ui";
import { useUiLocale, type UiLocale } from "../../i18n/UiLocaleProvider";
import { useUiTheme, type UiThemeMode } from "../../theme/UiThemeProvider";
import { SettingRow } from "./SettingsControls";
import { useDisplayPreferences, type DisplayPreferences } from "./displayPreferences";
import "./AppearanceSettings.css";

/** Existing theme and locale keys remain owned by their providers. Display choices use a separate local key. */
export function GeneralAppearanceSettings() {
  const { mode, setMode } = useUiTheme();
  const { locale, setLocale, t } = useUiLocale();
  const { value, update } = useDisplayPreferences();
  const [failed, setFailed] = useState(false);
  const id = useId();
  const save = (patch: Partial<DisplayPreferences>) => setFailed(!update(patch));
  return <>
    <SettingRow id="theme" label={t("外观", "Appearance")}>
      <SegmentedControl<UiThemeMode> hideLabel name={`${id}-theme`} label={t("外观", "Appearance")} value={mode} onValueChange={setMode}
        options={[{ value: "system", label: t("跟随系统", "System") }, { value: "light", label: t("浅色", "Light") }, { value: "dark", label: t("深色", "Dark") }]} />
    </SettingRow>
    <SettingRow id="locale" label={t("语言", "Language")}>
      <Select compact aria-label={t("语言", "Language")} value={locale} onChange={event => setLocale(event.target.value as UiLocale)}>
        <option value="zh-CN">简体中文</option><option value="en-US">English</option>
      </Select>
    </SettingRow>
    <SettingRow id="text-size" label={t("文字大小", "Text size")}>
      <SegmentedControl hideLabel name={`${id}-size`} label={t("文字大小", "Text size")} value={String(value.size)}
        options={[100, 110, 125, 150].map(size => ({ value: String(size), label: `${size}%` }))}
        onValueChange={size => save({ size: Number(size) as DisplayPreferences["size"] })} />
    </SettingRow>
    {(["ui", "code"] as const).map(kind => {
      const isCode = kind === "code";
      const label = isCode ? t("代码字体", "Code font") : t("界面字体", "Interface font");
      const font = isCode ? value.codeFont : value.uiFont;
      const weight = isCode ? value.codeWeight : value.uiWeight;
      return <SettingRow key={kind} id={isCode ? "code-font" : "ui-font"} label={label}>
        <div className="nt-settings-font">
          <Select compact id={`${id}-${kind}`} aria-label={label} value={font} onChange={event => {
            if (isCode) save({ codeFont: event.target.value as DisplayPreferences["codeFont"], codeWeight: "400" });
            else save({ uiFont: event.target.value as DisplayPreferences["uiFont"], uiWeight: "400" });
          }}>
            <option value="system">{t("系统默认", "System default")}</option>
            {isCode ? <><option value="cascadia">Cascadia Mono</option><option value="consolas">Consolas</option></>
              : <><option value="segoe">Segoe UI</option><option value="yahei">Microsoft YaHei</option></>}
          </Select>
          <Select compact aria-label={`${label} ${t("样式", "style")}`} value={weight} disabled={font === "system"}
            onChange={event => save(isCode ? { codeWeight: event.target.value as "400" | "700" } : { uiWeight: event.target.value as "400" | "700" })}>
            <option value="400">Regular</option><option value="700">Bold</option>
          </Select>
          <span className={isCode ? "nt-settings-font__code" : "nt-settings-font__ui"}>Aa {isCode ? "01 {}" : "文字"}</span>
        </div>
      </SettingRow>;
    })}
    <SettingRow id="provider-name" label={t("模型名带供应商", "Provider in model names")}>
      <SegmentedControl<DisplayPreferences["providerNames"]> hideLabel name={`${id}-provider-name`} label={t("模型名带供应商", "Provider in model names")} value={value.providerNames}
        onValueChange={providerNames => save({ providerNames })}
        options={[{ value: "off", label: t("关闭", "Off") }, { value: "except-custom", label: t("自定义名称不带供应商", "Except custom names") }, { value: "on", label: t("开启", "On") }]} />
    </SettingRow>
    {failed && <Notice tone="danger">{t("无法保存本地显示偏好，请检查浏览器存储权限。", "Could not save display preferences. Check storage permissions.")}</Notice>}
  </>;
}
