import { listen } from "@tauri-apps/api/event";
import { Settings } from "lucide-react";
import { useEffect, useState } from "react";
import { TabPanel, TabsRoot } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { AppShell } from "../shell/AppShell";
import { BackendConnectionForm } from "../settings/BackendConnectionForm";
import { DesktopBehaviorSettings } from "../settings/DesktopBehaviorSettings";
import { SettingsTabs, nativeSettingsCategories, type SettingsCategory } from "../settings/SettingsNavigation";
import { useDesktopPreferences } from "../settings/useDesktopPreferences";
import { useDesktopConnection } from "./useDesktopConnection";
import { GeneralAppearanceSettings } from "../settings/GeneralAppearanceSettings";
import { NativeAboutSettings } from "../settings/NativeAboutSettings";

/** Native operations live only in this trusted local window, not server-rendered content. */
export function DesktopConnectionApp() {
  const { t } = useUiLocale();
  const connection = useDesktopConnection();
  const preferences = useDesktopPreferences();
  const [category, setCategory] = useState<SettingsCategory>("backend");
  const [collapsed, setCollapsed] = useState(false);
  const [aboutLocked, setAboutLocked] = useState(false);
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<string>("gateway-settings-category", event => {
      if (nativeSettingsCategories.includes(event.payload as SettingsCategory)) setCategory(event.payload as SettingsCategory);
    }).then(stop => { if (disposed) stop(); else unlisten = stop; }).catch(() => {
      // Event delivery is optional in browser development; the local tabs still work.
    });
    return () => { disposed = true; unlisten?.(); };
  }, []);
  const current = connection.address ? new URL(connection.address).origin : "";
  return <TabsRoot className="nt-settings-navigation-root" value={category} onValueChange={next => { if (!connection.busy && !preferences.pending && !aboutLocked) setCategory(next as SettingsCategory); }}><AppShell productName="Gateway" navLabel="Gateway local settings" navItems={[]}
    utilityItems={[{ id: "settings", label: t("设置", "Settings"), icon: <Settings size={16} /> }]}
    activeNavId="settings" onNavigate={() => {}} railCollapsed={collapsed} onToggleRailCollapsed={() => setCollapsed(!collapsed)}
    title={t("设置", "Settings")} showLocaleToggle shellClassName="nt-settings-native-shell"
    headerLeading={<SettingsTabs category={category} onChange={setCategory} categories={nativeSettingsCategories} locked={connection.busy || preferences.pending || aboutLocked} />}>
    <div className="nt-settings-workspace"><TabPanel value={category} className="nt-settings-workspace__content">
      {category === "about" ? <NativeAboutSettings onInteractionChange={setAboutLocked} /> : category === "desktop" ? <><GeneralAppearanceSettings /><DesktopBehaviorSettings preferences={preferences} /></> : <BackendConnectionForm native current={current} localAddress={connection.localAddress}
          mode={connection.settings.mode} remote={connection.settings.serverUrl} busy={connection.operation || (connection.busy ? "apply" : "")}
          error={!!connection.error} message={connection.error ? t("未能连接目标后端，请检查地址和服务状态后重试。", "Could not connect. Check the backend address and service, then retry.") : connection.tested ? t("控制台可访问；管理权限需要在目标后端登录后验证。", "Console reachable. Management access requires sign-in on the target.") : ""}
          onMode={mode => connection.setSettings({ ...connection.settings, mode })}
          onRemote={serverUrl => connection.setSettings({ ...connection.settings, serverUrl })}
          onLocalPort={localPort => connection.setSettings({ ...connection.settings, localPort })}
          onTest={() => void connection.test(connection.settings)} onApply={() => void connection.connect(connection.settings)} />}
    </TabPanel></div>
  </AppShell></TabsRoot>;
}
