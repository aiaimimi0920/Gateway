/** Wire identity is independent of API protocol and selected credentials. */
export const REQUEST_CLIENT_PROFILES = ["auto", "codex_cli", "claude_cli", "workbuddy", "cpa", "cpa_claude_code"] as const;
export type RequestClientProfile = typeof REQUEST_CLIENT_PROFILES[number];

export function readRequestClientProfile(value: unknown): RequestClientProfile {
  return REQUEST_CLIENT_PROFILES.includes(value as RequestClientProfile) ? value as RequestClientProfile : "auto";
}

export function writeRequestClientProfile(provider: Record<string, unknown>, profile: RequestClientProfile): void {
  if (profile === "auto") delete provider.request_client_profile;
  else provider.request_client_profile = profile;
}
