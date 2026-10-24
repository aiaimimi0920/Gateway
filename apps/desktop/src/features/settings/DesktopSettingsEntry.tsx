import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { useUiTheme } from "../../theme/UiThemeProvider";
import type { NativeSettingsCategory } from "../../platform/desktopSettings";
import "./DesktopSettingsEntry.css";

/** Reserve a content-only slot for a trusted local child view, not another native window. */
export function DesktopSettingsEntry({ category, onInteractionChange }: {
  category: NativeSettingsCategory; onInteractionChange(locked: boolean): void;
}) {
  const { t, locale } = useUiLocale();
  const { resolved } = useUiTheme();
  const slot = useRef<HTMLDivElement>(null);
  const [failed, setFailed] = useState(false);
  const [ready, setReady] = useState(false);
  const [showLoading, setShowLoading] = useState(false);
  useEffect(() => {
    if (ready) { setShowLoading(false); return; }
    const timer = window.setTimeout(() => setShowLoading(true), 200);
    return () => window.clearTimeout(timer);
  }, [ready]);
  const latest = useRef({ category, resolved, locale, onInteractionChange });
  latest.current = { category, resolved, locale, onInteractionChange };
  const requestUpdate = useRef<() => void>(() => {});
  useEffect(() => {
    const bridge = window.__GATEWAY_WINDOW_CHROME__;
    let frame = 0;
    setFailed(!bridge?.showSettings);
    setReady(false);
    const update = () => {
      frame = 0;
      const rect = slot.current?.getBoundingClientRect();
      if (!rect || !bridge?.showSettings) return;
      const stage = slot.current?.closest(".nt-stage")?.getBoundingClientRect();
      // Parent scrolling must never slide the native child over the rail or category header.
      const x = Math.max(0, Math.round(rect.left), Math.ceil(stage?.left ?? 0));
      const y = Math.max(0, Math.round(rect.top), Math.ceil(stage?.top ?? 0));
      const width = Math.round(Math.min(rect.right, window.innerWidth, stage?.right ?? Infinity) - x);
      const height = Math.round(Math.min(rect.bottom, window.innerHeight, stage?.bottom ?? Infinity) - y);
      if (width <= 0 || height <= 0 || document.hidden) { bridge.send("settings-hide"); return; }
      const current = latest.current;
      bridge.showSettings({ category: current.category, x, y, width, height, theme: current.resolved, locale: current.locale });
    };
    const schedule = () => { if (!frame) frame = requestAnimationFrame(update); };
    requestUpdate.current = schedule;
    const busy = (event: Event) => latest.current.onInteractionChange((event as CustomEvent).detail === true);
    const error = () => setFailed(true);
    const loaded = () => { setReady(true); setFailed(false); };
    const observer = typeof ResizeObserver === "undefined" ? null : new ResizeObserver(schedule);
    if (slot.current) observer?.observe(slot.current);
    // A rail collapse moves the slot without necessarily resizing the viewport.
    const stage = slot.current?.closest(".nt-stage");
    if (stage) observer?.observe(stage);
    window.addEventListener("resize", schedule);
    document.addEventListener("scroll", schedule, { capture: true, passive: true });
    document.addEventListener("visibilitychange", schedule);
    window.addEventListener("gateway:settings-busy", busy);
    window.addEventListener("gateway:settings-error", error);
    window.addEventListener("gateway:settings-ready", loaded);
    schedule();
    return () => {
      cancelAnimationFrame(frame); observer?.disconnect();
      window.removeEventListener("resize", schedule);
      document.removeEventListener("scroll", schedule, true);
      document.removeEventListener("visibilitychange", schedule);
      window.removeEventListener("gateway:settings-busy", busy);
      window.removeEventListener("gateway:settings-error", error);
      window.removeEventListener("gateway:settings-ready", loaded);
      requestUpdate.current = () => {};
      bridge?.send("settings-hide"); latest.current.onInteractionChange(false);
    };
  }, []);
  // Category/theme changes update the retained local view, without hide/recreate or loading text flashes.
  useEffect(() => { requestUpdate.current(); }, [category, locale, resolved]);
  return <div ref={slot} className="nt-desktop-settings-slot" aria-busy={!ready && !failed}>
    {((!ready && showLoading) || failed) && <p className="nt-settings-host-note" role={failed ? "alert" : "status"}>{failed
      ? t("无法显示内嵌设置，请确认使用的是支持此功能的新版客户端。", "Could not display embedded settings. Use a client that supports this feature.")
      : t("加载中…", "Loading…")}</p>}
  </div>;
}
