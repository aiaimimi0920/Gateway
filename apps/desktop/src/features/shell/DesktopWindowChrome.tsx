import { useEffect } from "react";
import { Cable, Minus, Square, X } from "lucide-react";
import { IconButton } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { SettingsPanelLayout } from "../../platform/desktopSettings";
import "./DesktopWindowChrome.css";

type WindowAction = "ready" | "minimize" | "toggle-maximize" | "close" | "quit" | "connection" | "drag" | "settings-desktop" | "settings-hide" | "console";

declare global {
  interface Window {
    __GATEWAY_WINDOW_CHROME__?: { send(action: WindowAction): void; showSettings?(layout: SettingsPanelLayout): void };
  }
}

export function DesktopWindowChrome() {
  const { t } = useUiLocale();
  const bridge = window.__GATEWAY_WINDOW_CHROME__;
  useEffect(() => {
    if (!bridge) return;
    const keyboard = (event: KeyboardEvent) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
      const key = event.key.toLowerCase();
      const action = key === "g" && event.shiftKey ? "connection"
        : key === "q" && !event.shiftKey ? "quit" : null;
      if (!action) return;
      event.preventDefault();
      bridge.send(action);
    };
    const drag = (event: MouseEvent) => {
      if (event.button !== 0 || !(event.target instanceof Element)) return;
      if (!event.target.closest("[data-tauri-drag-region], [data-gateway-drag-region]") ||
        event.target.closest("button, a, input, select, textarea")) return;
      // Consume before Tauri's default drag handler: server pages have no IPC permissions.
      event.stopImmediatePropagation();
      event.preventDefault();
      bridge.send(event.detail === 2 ? "toggle-maximize" : "drag");
    };
    document.addEventListener("keydown", keyboard);
    document.addEventListener("mousedown", drag, true);
    bridge.send("ready");
    return () => {
      document.removeEventListener("keydown", keyboard);
      document.removeEventListener("mousedown", drag, true);
    };
  }, [bridge]);

  if (!bridge) return null;
  return <>
    <div className="nt-native-auth-drag" data-gateway-drag-region />
    <div className="nt-native-controls" role="group" aria-label={t("窗口操作", "Window controls")}>
      <IconButton compact variant="quiet" className="gateway-window-action" title={t("连接设置 (Ctrl+Shift+G)", "Connection (Ctrl+Shift+G)")}
        label={t("连接设置", "Connection settings")} onClick={() => bridge.send("connection")}><Cable size={16} aria-hidden="true" /></IconButton>
      <IconButton compact variant="quiet" className="gateway-window-action" title={t("最小化", "Minimize")}
        label={t("最小化", "Minimize")} onClick={() => bridge.send("minimize")}><Minus size={15} aria-hidden="true" /></IconButton>
      <IconButton compact variant="quiet" className="gateway-window-action" title={t("最大化或还原", "Maximize or restore")}
        label={t("最大化或还原", "Maximize or restore")} onClick={() => bridge.send("toggle-maximize")}><Square size={13} aria-hidden="true" /></IconButton>
      <IconButton compact variant="quiet" className="gateway-window-action" title={t("关闭", "Close")}
        label={t("关闭", "Close")} onClick={() => bridge.send("close")}><X size={15} aria-hidden="true" /></IconButton>
    </div>
  </>;
}
