import { useId } from "react";
import { useUiLocale, type UiLocale } from "../../i18n/UiLocaleProvider";
import { useUiTheme, type UiThemeMode } from "../../theme/UiThemeProvider";
import "./AppearanceSettings.css";

/** Existing preference keys and providers remain the sole persistence owners. */
export function AppearanceSettings() {
  const { mode, setMode } = useUiTheme();
  const { locale, setLocale, t } = useUiLocale();
  const id = useId();
  return <div className="nt-settings-fields">
    <div className="nt-setting-row">
      <label htmlFor={`${id}-theme`}>{t("主题", "Theme")}</label>
      <select className="nt-input" id={`${id}-theme`} value={mode} onChange={event => setMode(event.target.value as UiThemeMode)}>
        <option value="system">{t("跟随系统", "System")}</option>
        <option value="dark">{t("深色", "Dark")}</option>
        <option value="light">{t("浅色", "Light")}</option>
      </select>
    </div>
    <div className="nt-setting-row">
      <label htmlFor={`${id}-locale`}>{t("界面语言", "Language")}</label>
      <select className="nt-input" id={`${id}-locale`} value={locale} onChange={event => setLocale(event.target.value as UiLocale)}>
        <option value="zh-CN">简体中文</option><option value="en-US">English</option>
      </select>
    </div>
  </div>;
}