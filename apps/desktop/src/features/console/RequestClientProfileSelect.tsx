import { REQUEST_CLIENT_PROFILES, type ProviderRequestClientProfile, type RequestClientProfile } from "./requestClientProfile";

type Props = {
  value?: ProviderRequestClientProfile;
  disabled?: boolean;
  onChange(value: RequestClientProfile): void;
  t(zh: string, en: string): string;
};

export function RequestClientProfileSelect({ value = "auto", disabled, onChange, t }: Props) {
  const managed = value === "session" || value === "antigravity";
  return <label className="nt-field">
    <span>{t("客户端", "Client")}</span>
    <select className="nt-select" value={value} disabled={disabled || managed}
      onChange={(event) => {
        const profile = event.currentTarget.value as RequestClientProfile;
        if (!managed && REQUEST_CLIENT_PROFILES.includes(profile)) onChange(profile);
      }}>
      {managed ? <option value={value}>{value === "antigravity" ? "Antigravity" : t("站点会话", "Site session")}</option> : REQUEST_CLIENT_PROFILES.map((profile) => <option key={profile} value={profile}>
        {profile === "auto" ? t("自动", "Auto")
          : profile === "codex_cli" ? "codex" : profile === "claude_cli" ? "claude"
            : profile === "cpa_claude_code" ? "cpa-claude" : profile}
      </option>)}
    </select>
  </label>;
}
