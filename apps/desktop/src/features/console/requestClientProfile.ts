/** Wire identity is independent of API protocol and selected credentials. */
export const REQUEST_CLIENT_PROFILES = ["auto", "codex_cli", "claude_cli", "workbuddy", "cpa", "cpa_claude_code"] as const;
export type RequestClientProfile = typeof REQUEST_CLIENT_PROFILES[number];
/** Session identity is display-only; only the owning executor can change it. */
export type ProviderRequestClientProfile = RequestClientProfile | "session" | "antigravity";

export function readRequestClientProfile(value: unknown): RequestClientProfile {
  return REQUEST_CLIENT_PROFILES.includes(value as RequestClientProfile) ? value as RequestClientProfile : "auto";
}

/** Flow is authoritative for new providers; old documents retain their original field. */
export function readProviderRequestClientProfile(provider: Record<string, unknown>): ProviderRequestClientProfile {
  const flow = provider[Object.hasOwn(provider, "callFlow") ? "callFlow" : "call_flow"];
  if (flow && typeof flow === "object" && !Array.isArray(flow)) {
    const policy = flow as Record<string, unknown>;
    if (policy.special_adapter === "antigravity_oauth" || policy.fingerprint_adapter === "antigravity") return "antigravity";
    return ownsSessionFingerprint(policy) ? "session" : readRequestClientProfile(policy.fingerprint_adapter);
  }
  return readRequestClientProfile(provider[Object.hasOwn(provider, "requestClientProfile") ? "requestClientProfile" : "request_client_profile"]);
}

export function writeRequestClientProfile(provider: Record<string, unknown>, profile: RequestClientProfile): boolean {
  // Reject ambiguous aliases before any write; preserve the document's existing spelling.
  if (!REQUEST_CLIENT_PROFILES.includes(profile)
    || (Object.hasOwn(provider, "call_flow") && Object.hasOwn(provider, "callFlow"))
    || (Object.hasOwn(provider, "request_client_profile") && Object.hasOwn(provider, "requestClientProfile"))) return false;
  const flowKey = Object.hasOwn(provider, "callFlow") ? "callFlow" : "call_flow";
  const profileKey = Object.hasOwn(provider, "requestClientProfile") ? "requestClientProfile" : "request_client_profile";
  const flow = provider[flowKey];
  if (Object.hasOwn(provider, flowKey)) {
    if (!flow || typeof flow !== "object" || Array.isArray(flow)) return false;
    const policy = flow as Record<string, unknown>;
    if (ownsSessionFingerprint(policy) || (Object.hasOwn(policy, "fingerprint_adapter")
      && !REQUEST_CLIENT_PROFILES.includes(policy.fingerprint_adapter as RequestClientProfile))) return false;
    provider[flowKey] = { ...policy, fingerprint_adapter: profile };
  }
  if (profile === "auto") delete provider[profileKey];
  else provider[profileKey] = profile;
  return true;
}

function ownsSessionFingerprint(flow: Record<string, unknown>): boolean {
  return flow.fingerprint_adapter === "session" || flow.fingerprint_adapter === "antigravity"
    || flow.special_adapter === "antigravity_oauth"
    || ["basispoints", "gemini_web", "gemini_canvas", "chatgpt_web", "aistudio_web", "flowmusic_web", "flow_web", "stitch_web"].includes(flow.special_adapter as string);
}
