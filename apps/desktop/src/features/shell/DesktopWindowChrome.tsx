import { useEffect } from "react";
import { Cable, Minus, Square, X } from "lucide-react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import "./DesktopWindowChrome.css";

type WindowAction = "ready" | "minimize" | "toggle-maximize" | "close" | "connection" | "drag";

declare global {
  interface Window {
    __GATEWAY_WINDOW_CHROME__?: { send(action: WindowAction): void };
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
        : key === "q" && !event.shiftKey ? "close" : null;
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
      <button type="button" className="nt-window-control" title={t("连接设置 (Ctrl+Shift+G)", "Connection (Ctrl+Shift+G)")}
        aria-label={t("连接设置", "Connection settings")} onClick={() => bridge.send("connection")}><Cable size={16} /></button>
      <button type="button" className="nt-window-control" title={t("最小化", "Minimize")}
        aria-label={t("最小化", "Minimize")} onClick={() => bridge.send("minimize")}><Minus size={15} /></button>
      <button type="button" className="nt-window-control" title={t("最大化或还原", "Maximize or restore")}
        aria-label={t("最大化或还原", "Maximize or restore")} onClick={() => bridge.send("toggle-maximize")}><Square size={13} /></button>
      <button type="button" className="nt-window-control nt-window-control--close" title={t("关闭", "Close")}
        aria-label={t("关闭", "Close")} onClick={() => bridge.send("close")}><X size={15} /></button>
    </div>
  </>;
}
