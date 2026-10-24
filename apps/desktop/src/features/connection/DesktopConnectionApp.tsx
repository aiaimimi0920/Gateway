import { listen } from "@tauri-apps/api/event";
import { Settings } from "lucide-react";
import { useEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { AppShell } from "../shell/AppShell";
import { BackendConnectionForm } from "../settings/BackendConnectionForm";
import { DesktopBehaviorSettings } from "../settings/DesktopBehaviorSettings";
import { SettingsTabs, nativeSettingsCategories, type SettingsCategory } from "../settings/SettingsNavigation";
import { useDesktopPreferences } from "../settings/useDesktopPreferences";
import { useDesktopConnection } from "./useDesktopConnection";

/** Native operations live only in this trusted local window, not server-rendered content. */
export function DesktopConnectionApp() {
  const { t } = useUiLocale();
  const connection = useDesktopConnection();
  const preferences = useDesktopPreferences();
  const [category, setCategory] = useState<SettingsCategory>("backend");
  const [collapsed, setCollapsed] = useState(false);
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
  return <AppShell productName="Gateway" navLabel="Gateway local settings" navItems={[]}
    utilityItems={[{ id: "settings", label: t("设置", "Settings"), icon: <Settings size={16} /> }]}
    activeNavId="settings" onNavigate={() => {}} railCollapsed={collapsed} onToggleRailCollapsed={() => setCollapsed(!collapsed)}
    title={t("设置", "Settings")} showLocaleToggle shellClassName="nt-settings-native-shell"
    headerLeading={<SettingsTabs category={category} onChange={setCategory} categories={nativeSettingsCategories} locked={connection.busy || preferences.pending} />}>
    <div className="nt-settings-workspace"><section className="nt-settings-workspace__content" role="tabpanel">
      {category === "desktop" ? <DesktopBehaviorSettings preferences={preferences} /> : <BackendConnectionForm native current={current} localAddress="http://127.0.0.1:4200"
          mode={connection.settings.mode} remote={connection.settings.serverUrl} busy={connection.operation || (connection.busy ? "apply" : "")}
          error={!!connection.error} message={connection.error ? t("未能连接目标后端，请检查地址和服务状态后重试。", "Could not connect. Check the backend address and service, then retry.") : connection.tested ? t("控制台可访问；管理权限需要在目标后端登录后验证。", "Console reachable. Management access requires sign-in on the target.") : ""}
          onMode={mode => connection.setSettings({ ...connection.settings, mode })}
          onRemote={serverUrl => connection.setSettings({ ...connection.settings, serverUrl })}
          onTest={() => void connection.test(connection.settings)} onApply={() => void connection.connect(connection.settings)} />}
    </section></div>
  </AppShell>;
}
