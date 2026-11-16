import { useUiLocale } from "./UiLocaleProvider";
import { Button } from "@neuro/ui";

export type LanguageToggleButtonProps = {
  className?: string;
};

export function LanguageToggleButton({ className }: LanguageToggleButtonProps) {
  const { locale, toggleLocale, t } = useUiLocale();
  const nextLabel = locale === "zh-CN" ? "English" : "中文";

  return (
    <Button compact variant="quiet"
      className={className}
      aria-label={t("切换界面语言", "Switch interface language")}
      title={t("切换界面语言", "Switch interface language")}
      onClick={toggleLocale}
    >
      {nextLabel}
    </Button>
  );
}
