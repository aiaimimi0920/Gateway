import { useSyncExternalStore } from "react";

const key = "gateway-ui-display";
const changeEvent = "gateway:display-preferences";
export const interfaceFonts = {
  system: '"Bahnschrift", "Segoe UI", "PingFang SC", "Microsoft YaHei", sans-serif',
  segoe: '"Segoe UI", "Microsoft YaHei", sans-serif',
  yahei: '"Microsoft YaHei", sans-serif',
};
export const codeFonts = {
  system: '"Cascadia Mono", "JetBrains Mono", Consolas, monospace',
  cascadia: '"Cascadia Mono", Consolas, monospace',
  consolas: 'Consolas, monospace',
};
export type DisplayPreferences = {
  providerNames: "off" | "except-custom" | "on";
  size: 100 | 110 | 125 | 150; uiFont: keyof typeof interfaceFonts; codeFont: keyof typeof codeFonts;
  uiWeight: "400" | "700"; codeWeight: "400" | "700";
};
const defaults: DisplayPreferences = { providerNames: "off", size: 100, uiFont: "system", codeFont: "system", uiWeight: "400", codeWeight: "400" };
let cached = defaults;
let rawCache: string | null | undefined;
function snapshot(): DisplayPreferences {
  try {
    const raw = localStorage.getItem(key);
    if (raw === rawCache) return cached;
    rawCache = raw;
    const value = JSON.parse(raw ?? "null");
    cached = { ...defaults };
    if (!value || typeof value !== "object") return cached;
    if (["off", "except-custom", "on"].includes(value.providerNames)) cached.providerNames = value.providerNames;
    if ([100, 110, 125, 150].includes(value.size)) cached.size = value.size;
    if (Object.hasOwn(interfaceFonts, value.uiFont)) cached.uiFont = value.uiFont;
    if (Object.hasOwn(codeFonts, value.codeFont)) cached.codeFont = value.codeFont;
    if (cached.uiFont !== "system" && value.uiWeight === "700") cached.uiWeight = "700";
    if (cached.codeFont !== "system" && value.codeWeight === "700") cached.codeWeight = "700";
  } catch { cached = defaults; }
  return cached;
}
function subscribe(listener: () => void) {
  window.addEventListener("storage", listener); window.addEventListener(changeEvent, listener);
  return () => { window.removeEventListener("storage", listener); window.removeEventListener(changeEvent, listener); };
}
function apply(value: DisplayPreferences) {
  const root = document.documentElement;
  root.style.setProperty("--nt-ui-font", interfaceFonts[value.uiFont]);
  root.style.setProperty("--nt-font-mono", codeFonts[value.codeFont]);
  root.style.setProperty("--nt-ui-weight", value.uiWeight);
  root.style.setProperty("--nt-code-weight", value.codeWeight);
  // Keep rem units standard; compact control sizes belong to the app styles.
  root.style.fontSize = `${16 * value.size / 100}px`;
}
export function initializeDisplayPreferences() {
  apply(snapshot());
  subscribe(() => apply(snapshot()));
}
export function useDisplayPreferences() {
  const value = useSyncExternalStore(subscribe, snapshot);
  const update = (patch: Partial<DisplayPreferences>): boolean => {
    try { localStorage.setItem(key, JSON.stringify({ ...value, ...patch })); }
    catch { return false; }
    apply(snapshot()); window.dispatchEvent(new Event(changeEvent)); return true;
  };
  return { value, update };
}
