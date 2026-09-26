import type { ReactNode } from "react";
import { Languages, Monitor, Moon, Palette, Sun } from "lucide-react";
import { useUiLocale, type UiLocale } from "../../i18n/UiLocaleProvider";
import { useUiTheme, type UiThemeMode } from "../../theme/UiThemeProvider";
import { SettingsSection } from "./SettingsSection";

type ThemeOption = { id: UiThemeMode; label: string; hint: string; icon: ReactNode };
type LocaleOption = { id: UiLocale; label: string; hint: string };

/**
 * Theme and language sections shared by the Tauri launcher settings page and the
 * web console settings workspace, so both shells expose the same controls.
 */
export function AppearanceSettings() {
  const { mode, resolved, setMode } = useUiTheme();
  const { locale, setLocale, t } = useUiLocale();

  const themeOptions: ThemeOption[] = [
    {
      id: "system",
      label: t("跟随系统", "System"),
      hint: t("按操作系统的明暗设置自动切换", "Follows the operating system preference"),
      icon: <Monitor size={17} aria-hidden="true" />,
    },
    {
      id: "dark",
      label: t("深色", "Dark"),
      hint: t("默认的 Neuro 深色信号面板", "The default Neuro dark signal panel"),
      icon: <Moon size={17} aria-hidden="true" />,
    },
    {
      id: "light",
      label: t("浅色", "Light"),
      hint: t("高亮环境下的浅色排版", "Light typography for bright rooms"),
      icon: <Sun size={17} aria-hidden="true" />,
    },
  ];

  const localeOptions: LocaleOption[] = [
    { id: "zh-CN", label: t("简体中文", "Chinese"), hint: "zh-CN" },
    { id: "en-US", label: t("英文", "English"), hint: "en-US" },
  ];

  return (
    <>
      <SettingsSection label={t("主题", "Theme")} icon={<Palette size={18} />}>
        <p className="nt-settings-hint">
          {t(
            "主题只影响桌面壳层和网页控制台的显示，不改变 Gateway 本体的无 UI 运行方式。",
            "The theme only affects this shell and the web console; the headless Gateway runtime is untouched.",
          )}
        </p>

        <div className="nt-option-grid" role="radiogroup" aria-label={t("主题", "Theme")}>
          {themeOptions.map((option) => {
            const active = option.id === mode;
            return (
              <button
                key={option.id}
                className={`nt-option-card${active ? " nt-option-card--active" : ""}`}
                type="button"
                role="radio"
                aria-checked={active}
                onClick={() => setMode(option.id)}
              >
                <span className="nt-option-card__icon" aria-hidden="true">
                  {option.icon}
                </span>
                <span className="nt-option-card__copy">
                  <strong>{option.label}</strong>
                  <small>{option.hint}</small>
                </span>
              </button>
            );
          })}
        </div>

        <dl className="nt-mini-list">
          <div>
            <dt>{t("当前生效", "Resolved")}</dt>
            <dd>{resolved === "light" ? t("浅色", "Light") : t("深色", "Dark")}</dd>
          </div>
        </dl>
      </SettingsSection>

      <SettingsSection label={t("界面语言", "Interface language")} icon={<Languages size={18} />}>
        <div className="nt-option-grid" role="radiogroup" aria-label={t("界面语言", "Interface language")}>
          {localeOptions.map((option) => {
            const active = option.id === locale;
            return (
              <button
                key={option.id}
                className={`nt-option-card${active ? " nt-option-card--active" : ""}`}
                type="button"
                role="radio"
                aria-checked={active}
                onClick={() => setLocale(option.id)}
              >
                <span className="nt-option-card__icon" aria-hidden="true">
                  <Languages size={17} />
                </span>
                <span className="nt-option-card__copy">
                  <strong>{option.label}</strong>
                  <small>{option.hint}</small>
                </span>
              </button>
            );
          })}
        </div>
      </SettingsSection>
    </>
  );
}
