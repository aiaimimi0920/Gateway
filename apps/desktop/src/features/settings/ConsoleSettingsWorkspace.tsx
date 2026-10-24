import { useEffect, useState } from "react";
import { AppearanceSettings } from "./AppearanceSettings";
import { GatewayDiagnosticsSettings } from "./GatewayDiagnosticsSettings";
import { ManagementSecuritySettings } from "./ManagementSecuritySettings";
import { DesktopSettingsEntry } from "./DesktopSettingsEntry";
import { SettingsTabs, useSettingsNavigation, type SettingsCategory } from "./SettingsNavigation";
import "./ConsoleSettingsWorkspace.css";

/** Server content never receives native IPC access; native forms occupy an isolated local child view. */
export function ConsoleSettingsWorkspace() {
  const navigation = useSettingsNavigation();
  const [localCategory, setLocalCategory] = useState<SettingsCategory>("appearance");
  const [localLocked, setLocalLocked] = useState(false);
  const category = navigation?.category ?? localCategory;
  const locked = navigation?.locked ?? localLocked;
  const setLocked = navigation?.setLocked ?? setLocalLocked;
  useEffect(() => () => setLocked(false), [setLocked]);
  const nativeCategory = category === "backend" || category === "desktop";
  return <div className="nt-settings-workspace">
    {!navigation && <SettingsTabs category={category} locked={locked} onChange={setLocalCategory} />}
    <section className={`nt-settings-workspace__content${category === "security" ? " nt-settings-workspace__content--security" : ""}`} role="tabpanel">
      {category === "appearance" ? <AppearanceSettings /> :
        category === "security" ? <ManagementSecuritySettings onInteractionChange={setLocked} /> :
        category === "diagnostics" ? <GatewayDiagnosticsSettings onInteractionChange={setLocked} /> :
        nativeCategory && window.__GATEWAY_WINDOW_CHROME__ ? <DesktopSettingsEntry category={category} onInteractionChange={setLocked} /> :
        null}
    </section>
  </div>;
}
