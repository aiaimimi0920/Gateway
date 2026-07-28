import { useUiLocale } from "./UiLocaleProvider";

export type LanguageToggleButtonProps = {
  className?: string;
};

export function LanguageToggleButton({ className }: LanguageToggleButtonProps) {
  const { locale, toggleLocale, t } = useUiLocale();
  const nextLabel = locale === "zh-CN" ? "English" : "中文";

  return (
    <button
      className={className ?? "nt-btn nt-btn--outline"}
      type="button"
      aria-label={t("切换界面语言", "Switch interface language")}
      title={t("切换界面语言", "Switch interface language")}
      onClick={toggleLocale}
    >
      {nextLabel}
    </button>
  );
}
