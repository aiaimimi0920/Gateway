import { useState } from "react";
import { SearchCombobox } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useSettingsNavigation } from "./SettingsNavigation";
import { matchesSetting, settingsCatalog, settingsCategoryTitles } from "./settingsCatalog";

/** Only the static public catalog is searchable; never index user data or drafts. */
export function SettingsSearch() {
  const navigation = useSettingsNavigation();
  const { t } = useUiLocale();
  const [query, setQuery] = useState("");
  const needle = query.trim().toLocaleLowerCase();
  const results = needle ? settingsCatalog.filter(item => matchesSetting(item, needle)).slice(0, 12) : [];
  if (!navigation) return null;
  return <SearchCombobox compact className="nt-settings-search" label={t("查找设置", "Find settings")}
    placeholder={t("查找设置", "Find settings")} value={query} onValueChange={setQuery} maxLength={80}
    clearLabel={t("清空搜索", "Clear search")} emptyLabel={t("没有找到相关设置", "No matching settings")}
    disabled={navigation.locked} onExpandedChange={navigation.setSearchOpen}
    options={results.map(item => ({ value: item.id, label: t(...item.title), description: t(...settingsCategoryTitles[item.category]) }))}
    onSelect={id => {
      const item = results.find(result => result.id === id);
      if (!item || navigation.locked) return;
      navigation.locate(item.category, item.id); setQuery("");
    }} />;
}
