import { useSettingsCategoryLabels, useSettingsNavigation } from "../settings/SettingsNavigation";

// Read the same selected category as the settings panel, including search navigation.
export function ShellPageTitle({ title, settings }: { title: string; settings: boolean }) {
  const navigation = useSettingsNavigation();
  const labels = useSettingsCategoryLabels();
  return <h1 className="nt-page-title">
    {settings && navigation ? <>
      <span className="nt-page-title__parent">{title}</span>
      <span className="nt-page-title__separator" aria-hidden="true">/</span>
      <span aria-current="page">{labels[navigation.category]}</span>
    </> : <span aria-current="page">{title}</span>}
  </h1>;
}
