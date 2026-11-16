import { Fragment, useId } from "react";
import { Button, Input, SegmentedControl, Select } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { SettingRow } from "./SettingsControls";
import { settingsCatalog, type PublicSettingsCategory, type SettingsItem } from "./settingsCatalog";

/** A disabled capability is not a persisted preference or an assertion about runtime state. */
export function UnavailableSetting({ item, reason }: { item: SettingsItem; reason?: string }) {
  const { t } = useUiLocale();
  const label = t(...item.title);
  const id = useId();
  return <SettingRow id={item.id} label={label} note={reason ?? t("当前版本尚未提供此设置", "Not available in this version")}>
    <fieldset className="nt-settings-unavailable" disabled aria-label={label}>
      {item.choices && (item.choiceLayout !== "segmented" && (item.choices.length > 4 || item.choices.some(choice => t(...choice).length > 8))
        ? <Select compact aria-label={label} value="" disabled><option value="">—</option>{item.choices.map(choice => <option key={choice[1]} value={choice[1]}>{t(...choice)}</option>)}</Select>
        : <SegmentedControl hideLabel name={id} label={label} value="" options={item.choices.map(choice => ({ value: choice[1], label: t(...choice) }))} disabled onValueChange={() => {}} />)}
      {item.input !== undefined && <Input compact aria-label={label} placeholder={item.input} disabled />}
      {item.action && <Button compact disabled>{t(...item.action)}</Button>}
    </fieldset>
  </SettingRow>;
}

export function UnavailableSettings({ category, ids, reason }: {
  category?: PublicSettingsCategory; ids?: readonly string[]; reason?: string;
}) {
  const { t } = useUiLocale();
  return <>{settingsCatalog.filter(item => ids ? ids.includes(item.id) : item.category === category).map(item =>
    <Fragment key={item.id}>{item.group && <h2 className="nt-settings-group">{t(...item.group)}</h2>}
      <UnavailableSetting item={item} reason={reason} /></Fragment>)}</>;
}
