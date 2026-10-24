import { Cable, Monitor, Palette, ScrollText, ShieldCheck } from "lucide-react";
import { createContext, useContext, useState, type ReactNode } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import "./ConsoleSettingsWorkspace.css";

export type SettingsCategory = "appearance" | "backend" | "desktop" | "security" | "diagnostics";
export const settingsCategories = ["appearance", "backend", "desktop", "security", "diagnostics"] as const;
export const sharedSettingsCategories: readonly SettingsCategory[] = ["appearance", "security", "diagnostics"];
export const nativeSettingsCategories: readonly SettingsCategory[] = ["backend", "desktop"];
function visibleCategories() { return window.__GATEWAY_WINDOW_CHROME__ ? settingsCategories : sharedSettingsCategories; }
const SettingsNavigation = createContext<ReturnType<typeof useNavigationState> | null>(null);
function useNavigationState() {
  const [category, updateCategory] = useState<SettingsCategory>("appearance");
  const setCategory = (value: SettingsCategory) => { if (visibleCategories().includes(value)) updateCategory(value); };
  const [locked, setLocked] = useState(false);
  return { category, setCategory, locked, setLocked };
}
export function SettingsNavigationProvider({ children }: { children: ReactNode }) {
  const value = useNavigationState();
  return <SettingsNavigation.Provider value={value}>{children}</SettingsNavigation.Provider>;
}
export function useSettingsNavigation() { return useContext(SettingsNavigation); }

export function SettingsTabs({ category, onChange, locked = false, categories = visibleCategories() }: {
  category: SettingsCategory; onChange(value: SettingsCategory): void; locked?: boolean;
  categories?: readonly SettingsCategory[];
}) {
  const { t } = useUiLocale();
  const labels = { appearance: t("界面偏好", "Appearance"), backend: t("后端连接", "Backend"),
    desktop: t("桌面行为", "Desktop"), security: t("管理安全", "Security"), diagnostics: t("诊断与日志", "Diagnostics") };
  const icons = { appearance: Palette, backend: Cable, desktop: Monitor, security: ShieldCheck, diagnostics: ScrollText };
  return <div className="nt-settings-tabs" role="tablist" aria-label={t("设置分类", "Settings categories")}>
    {categories.map((value, index) => {
      const Icon = icons[value];
      return <button type="button" role="tab" key={value} aria-selected={category === value}
        aria-current={category === value ? "page" : undefined} tabIndex={category === value ? 0 : -1}
        disabled={locked && category !== value} onClick={() => onChange(value)}
        onKeyDown={event => {
          if (locked || !["ArrowRight", "ArrowLeft", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          const count = categories.length;
          const next = event.key === "Home" ? 0 : event.key === "End" ? count - 1 : (index + (event.key === "ArrowLeft" ? count - 1 : 1)) % count;
          onChange(categories[next]);
          (event.currentTarget.parentElement?.children[next] as HTMLElement)?.focus();
        }}><Icon size={17} aria-hidden="true" />{labels[value]}</button>;
    })}
  </div>;
}
export function SettingsHeaderSlot() {
  const navigation = useSettingsNavigation();
  return navigation && <SettingsTabs category={navigation.category} locked={navigation.locked} onChange={navigation.setCategory} />;
}
