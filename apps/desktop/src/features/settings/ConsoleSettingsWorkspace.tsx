import { useEffect, useState } from "react";
import { TabPanel, TabsRoot } from "@neuro/ui";
import { GeneralAppearanceSettings } from "./GeneralAppearanceSettings";
import { GatewayDiagnosticsSettings } from "./GatewayDiagnosticsSettings";
import { ManagementSecuritySettings } from "./ManagementSecuritySettings";
import { DesktopSettingsEntry } from "./DesktopSettingsEntry";
import { SettingsTabs, useSettingsNavigation, type SettingsCategory } from "./SettingsNavigation";
import "./ConsoleSettingsWorkspace.css";
import { UnavailableSettings } from "./UnavailableSettings";
import { SettingRow } from "./SettingsControls";
import { AboutSettings } from "./AboutSettings";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useGatewayHost } from "../../platform/HostProvider";
import { GatewayProxySettings } from "./GatewayProxySettings";
import { GatewayLanSharingSettings } from "./GatewayLanSharingSettings";
import { GatewayBackupSettings } from "./GatewayBackupSettings";

/** Server content never receives native IPC access; native forms occupy an isolated local child view. */
export function ConsoleSettingsWorkspace() {
  const navigation = useSettingsNavigation();
  const { t } = useUiLocale();
  const host = useGatewayHost();
  const [localCategory, setLocalCategory] = useState<SettingsCategory>("appearance");
  const [nativeLocked, setNativeLocked] = useState(false);
  const [managementLocked, setManagementLocked] = useState(false);
  const [proxyLocked, setProxyLocked] = useState(false);
  const [sharingLocked, setSharingLocked] = useState(false);
  const category = navigation?.category ?? localCategory;
  const locked = nativeLocked || managementLocked || proxyLocked || sharingLocked;
  const setLocked = navigation?.setLocked;
  useEffect(() => { setLocked?.(locked); return () => setLocked?.(false); }, [locked, setLocked]);
  useEffect(() => {
    if (!navigation?.target) return;
    const frame = requestAnimationFrame(() => {
      const id = ["startup", "awake"].includes(navigation.target?.id ?? "") && window.__GATEWAY_WINDOW_CHROME__ ? "close" : navigation.target?.id;
      const row = document.querySelector<HTMLElement>(`[data-setting-id="${id}"]`);
      row?.scrollIntoView?.({ block: "center", behavior: "instant" }); row?.focus({ preventScroll: true });
    });
    return () => cancelAnimationFrame(frame);
  }, [navigation?.target]);
  const native = !!window.__GATEWAY_WINDOW_CHROME__;
  const suspended = managementLocked || proxyLocked || sharingLocked || !!navigation?.searchOpen;
  const content = <div className="nt-settings-workspace">
    {!navigation && <SettingsTabs category={category} locked={locked} onChange={setLocalCategory} />}
    <TabPanel value={category} className={`nt-settings-workspace__content nt-settings-workspace__content--${category}`}>
      {category === "appearance" ? <>
        <GeneralAppearanceSettings />
        {native ? <DesktopSettingsEntry category="desktop" onInteractionChange={setNativeLocked} suspended={suspended} />
          : <UnavailableSettings ids={["awake", "close", "startup"]} reason={t("仅桌面客户端可用", "Desktop client only")} />}
      </> : category === "network" ? <>
        <GatewayProxySettings onInteractionChange={setProxyLocked} />
        {native ? <DesktopSettingsEntry category="backend" onInteractionChange={setNativeLocked} suspended={suspended} />
          : <SettingRow id="endpoint" label={t("网关地址", "Gateway address")} note={t("浏览器连接当前站点；切换本机或远程请使用桌面客户端", "Browser uses this site. Use the desktop client to switch gateways.")}><code className="nt-settings-address">{host.apiOrigin || window.location.origin}</code></SettingRow>}
        <div data-setting-id="management-keys" tabIndex={-1}><ManagementSecuritySettings onInteractionChange={setManagementLocked} /></div>
        <GatewayLanSharingSettings onInteractionChange={setSharingLocked} disabled={nativeLocked || managementLocked || proxyLocked} />
      </> : category === "diagnostics" ? <>
        <GatewayDiagnosticsSettings onInteractionChange={setManagementLocked} />
      </> : category === "backup" ? <GatewayBackupSettings onInteractionChange={setManagementLocked} />
          : category === "about" ? (native ? <DesktopSettingsEntry category="about" onInteractionChange={setNativeLocked} suspended={suspended} /> : <AboutSettings />) : null}
    </TabPanel>
  </div>;
  return navigation ? content : <TabsRoot className="nt-settings-navigation-root" value={category} onValueChange={next => { if (!locked) setLocalCategory(next as SettingsCategory); }}>{content}</TabsRoot>;
}
