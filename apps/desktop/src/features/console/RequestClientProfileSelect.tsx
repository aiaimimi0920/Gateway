import { REQUEST_CLIENT_PROFILES, type RequestClientProfile } from "./requestClientProfile";

type Props = {
  value?: RequestClientProfile;
  disabled?: boolean;
  onChange(value: RequestClientProfile): void;
  t(zh: string, en: string): string;
};

export function RequestClientProfileSelect({ value = "auto", disabled, onChange, t }: Props) {
  return <label className="nt-field">
    <span>{t("上游客户端参数", "Upstream client parameters")}</span>
    <select className="nt-select" value={value} disabled={disabled}
      onChange={(event) => onChange(event.currentTarget.value as RequestClientProfile)}>
      {REQUEST_CLIENT_PROFILES.map((profile) => <option key={profile} value={profile}>
        {profile === "auto" ? t("自动（Claude → Claude CLI，其余 → Codex CLI）", "Automatic (Claude → Claude CLI; others → Codex CLI)")
          : profile === "codex_cli" ? "Codex CLI" : profile === "claude_cli" ? "Claude CLI"
            : profile === "workbuddy" ? "WorkBuddy" : profile === "cpa_claude_code"
              ? t("CPA Claude Code 完整身份（显式启用）", "CPA Claude Code full identity (opt-in)")
              : t("CPA 兼容", "CPA compatible")}
      </option>)}
    </select>
  </label>;
}
