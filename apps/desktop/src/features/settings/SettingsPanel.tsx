import { Info } from "lucide-react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { GatewayDesktopState } from "../../lib/types";
import { AppearanceSettings } from "./AppearanceSettings";
import { SettingsSection } from "./SettingsSection";

export function SettingsPanel({ state }: { state: GatewayDesktopState }) {
  const { t } = useUiLocale();

  return (
    <div className="nt-settings-page">
      <div className="nt-settings-accordion">
        <AppearanceSettings />

        <SettingsSection label={t("壳层信息", "Shell information")} icon={<Info size={18} />}>
          <dl className="nt-mini-list">
            <div>
              <dt>Profile</dt>
              <dd>{state.draftProfile.name}</dd>
            </div>
            <div>
              <dt>Base URL</dt>
              <dd>{state.baseUrl}</dd>
            </div>
            <div>
              <dt>Runtime</dt>
              <dd>{state.isTauriAvailable ? "Tauri" : t("浏览器预览", "Browser preview")}</dd>
            </div>
          </dl>
        </SettingsSection>
      </div>
    </div>
  );
}
