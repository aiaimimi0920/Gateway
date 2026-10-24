/** Only presentation metadata crosses the server/native boundary; never credentials or settings. */
export type NativeSettingsCategory = "backend" | "desktop";
export type SettingsPanelLayout = {
  category: NativeSettingsCategory;
  x: number; y: number; width: number; height: number;
  theme: "dark" | "light"; locale: "zh-CN" | "en-US";
};
export type EmbeddedSettingsContext = Pick<SettingsPanelLayout, "category" | "theme" | "locale"> & { current: string; revision: number };

declare global {
  interface Window { __GATEWAY_EMBEDDED_SETTINGS__?: EmbeddedSettingsContext; }
}
