import { REQUEST_CLIENT_PROFILES, type RequestClientProfile } from "./requestClientProfile";

type Props = {
  value?: RequestClientProfile;
  disabled?: boolean;
  onChange(value: RequestClientProfile): void;
  t(zh: string, en: string): string;
};

export function RequestClientProfileSelect({ value = "auto", disabled, onChange, t }: Props) {
  return <label className="nt-field">
    <span>{t("客户端", "Client")}</span>
    <select className="nt-select" value={value} disabled={disabled}
      onChange={(event) => onChange(event.currentTarget.value as RequestClientProfile)}>
      {REQUEST_CLIENT_PROFILES.map((profile) => <option key={profile} value={profile}>
        {profile === "auto" ? t("自动", "Auto")
          : profile === "codex_cli" ? "codex" : profile === "claude_cli" ? "claude"
            : profile === "cpa_claude_code" ? "cpa-claude" : profile}
      </option>)}
    </select>
  </label>;
}
