import { invoke } from "@tauri-apps/api/core";
import { useEffect, useLayoutEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useUiTheme } from "../../theme/UiThemeProvider";
import type { EmbeddedSettingsContext } from "../../platform/desktopSettings";
import { useDesktopConnection } from "../connection/useDesktopConnection";
import { BackendConnectionForm } from "./BackendConnectionForm";
import { DesktopBehaviorSettings } from "./DesktopBehaviorSettings";
import { useDesktopPreferences } from "./useDesktopPreferences";
import "./AppearanceSettings.css";
import "./DesktopSettingsEntry.css";

/** This entry is loaded only from packaged assets in the trusted content-only webview. */
export function EmbeddedDesktopSettings({ context: initialContext }: { context: EmbeddedSettingsContext }) {
  const [context, setContext] = useState(initialContext);
  const { locale, setLocale } = useUiLocale();
  const { resolved, setMode } = useUiTheme();
  useLayoutEffect(() => {
    const update = (event: Event) => setContext(current => {
      const next = (event as CustomEvent<EmbeddedSettingsContext>).detail;
      return next.revision > current.revision ? next : current;
    });
    window.addEventListener("gateway:settings-context", update);
    return () => window.removeEventListener("gateway:settings-context", update);
  }, []);
  useLayoutEffect(() => { setLocale(context.locale); setMode(context.theme); }, [context.locale, context.theme, setLocale, setMode]);
  useEffect(() => {
    if (locale === context.locale && resolved === context.theme) {
      void invoke("gateway_settings_ready", { revision: context.revision }).catch(() => {});
    }
  }, [context.revision, context.locale, context.theme, locale, resolved]);
  return <div className="nt-embedded-settings nt-settings-workspace">
    {context.category === "backend" ? <EmbeddedBackend current={context.current} revision={context.revision} /> : <EmbeddedPreferences revision={context.revision} />}
  </div>;
}

function useParentLock(pending: boolean, revision: number) {
  useEffect(() => {
    void invoke("set_gateway_settings_busy", { busy: pending, revision }).catch(() => {});
  }, [pending, revision]);
  useEffect(() => () => { void invoke("set_gateway_settings_busy", { busy: false, revision }).catch(() => {}); }, [revision]);
}

function EmbeddedPreferences({ revision }: { revision: number }) {
  const preferences = useDesktopPreferences();
  useParentLock(preferences.pending, revision);
  return <DesktopBehaviorSettings preferences={preferences} />;
}

function EmbeddedBackend({ current, revision }: { current: string; revision: number }) {
  const { t } = useUiLocale();
  // Opening settings must not reconnect, start a sidecar or replace the current console.
  const connection = useDesktopConnection(false, current);
  useParentLock(connection.busy, revision);
  return <BackendConnectionForm native current={connection.address} localAddress="http://127.0.0.1:4200"
    mode={connection.settings.mode} remote={connection.settings.serverUrl} busy={connection.operation || (connection.busy ? "apply" : "")}
    error={!!connection.error} message={connection.error ? t("未能连接目标后端，请检查地址和服务状态后重试。", "Could not connect. Check the backend address and service, then retry.")
      : connection.tested ? t("控制台可访问；管理权限需要在目标后端登录后验证。", "Console reachable. Management access requires sign-in on the target.") : ""}
    onMode={mode => connection.setSettings({ ...connection.settings, mode })}
    onRemote={serverUrl => connection.setSettings({ ...connection.settings, serverUrl })}
    onTest={() => void connection.test(connection.settings)} onApply={() => void connection.connect(connection.settings)} />;
}
