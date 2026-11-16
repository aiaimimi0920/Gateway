import { createContext, useContext, useState, type ReactNode } from "react";
import { Tab, TabList, TabsRoot } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import "./ConsoleSettingsWorkspace.css";
import type { PublicSettingsCategory } from "./settingsCatalog";

// backend/desktop remain native bridge identifiers, not extra public categories.
export type SettingsCategory = PublicSettingsCategory | "backend" | "desktop";
export const settingsCategories = ["appearance", "network", "diagnostics", "backup", "about"] as const;
export const sharedSettingsCategories: readonly SettingsCategory[] = settingsCategories;
export const nativeSettingsCategories: readonly SettingsCategory[] = ["backend", "desktop", "about"];
function visibleCategories(): readonly SettingsCategory[] { return settingsCategories; }
const SettingsNavigation = createContext<ReturnType<typeof useNavigationState> | null>(null);
function useNavigationState() {
  const [category, updateCategory] = useState<SettingsCategory>("appearance");
  const setCategory = (value: SettingsCategory) => { if (visibleCategories().includes(value)) updateCategory(value); };
  const [locked, setLocked] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [target, setTarget] = useState<{ id: string; sequence: number } | null>(null);
  const locate = (value: PublicSettingsCategory, id: string) => {
    if (locked) return;
    updateCategory(value); setTarget(current => ({ id, sequence: (current?.sequence ?? 0) + 1 }));
  };
  return { category, setCategory, locked, setLocked, searchOpen, setSearchOpen, target, locate };
}
export function SettingsNavigationProvider({ children }: { children: ReactNode }) {
  const value = useNavigationState();
  return <SettingsNavigation.Provider value={value}>
    <TabsRoot className="nt-settings-navigation-root" value={value.category} onValueChange={next => { if (!value.locked) value.setCategory(next as SettingsCategory); }}>{children}</TabsRoot>
  </SettingsNavigation.Provider>;
}
export function useSettingsNavigation() { return useContext(SettingsNavigation); }
export function useSettingsCategoryLabels() {
  const { t } = useUiLocale();
  return { appearance: t("常规", "General"), network: t("网络与共享", "Network & sharing"),
    diagnostics: t("可观测性", "Observability"), backup: t("同步与备份", "Sync & backup"),
    about: t("关于", "About"), backend: t("网络与共享", "Network & sharing"), desktop: t("常规", "General") };
}

export function SettingsTabs({ category, onChange, locked = false, categories = visibleCategories() }: {
  category: SettingsCategory; onChange(value: SettingsCategory): void; locked?: boolean;
  categories?: readonly SettingsCategory[];
}) {
  const { t } = useUiLocale();
  const labels = useSettingsCategoryLabels();
  return <TabList className="nt-settings-tabs" label={t("设置分类", "Settings categories")}>
    {categories.map(value => {
      return <Tab value={value} key={value}
        aria-current={category === value ? "page" : undefined}
        disabled={locked && category !== value} onClick={() => onChange(value)}
        >{labels[value]}</Tab>;
    })}
  </TabList>;
}
export function SettingsHeaderSlot() {
  const navigation = useSettingsNavigation();
  return navigation && <SettingsTabs category={navigation.category} locked={navigation.locked} onChange={navigation.setCategory} />;
}
