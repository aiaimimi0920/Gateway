import type { GatewayEnvEntry, GatewayProfile } from "./types";

function normalizeText(value?: string | null): string {
  return value?.trim() ?? "";
}

function normalizeExtraEnv(extraEnv: GatewayEnvEntry[]): GatewayEnvEntry[] {
  return extraEnv
    .map((entry) => ({
      key: entry.key.trim(),
      value: entry.value,
    }))
    .sort((left, right) => left.key.localeCompare(right.key));
}

export function normalizeGatewayProfileForComparison(profile: GatewayProfile): GatewayProfile {
  return {
    name: profile.name.trim(),
    runtimeRole: profile.runtimeRole,
    gatewayManagementToken: normalizeText(profile.gatewayManagementToken),
    port: profile.port,
    gatewayRedisUrl: normalizeText(profile.gatewayRedisUrl),
    gatewayDatabaseUrl: normalizeText(profile.gatewayDatabaseUrl),
    gatewayRoutesFile: normalizeText(profile.gatewayRoutesFile),
    logLevel: normalizeText(profile.logLevel),
    workingDirectory: normalizeText(profile.workingDirectory),
    extraEnv: normalizeExtraEnv(profile.extraEnv),
  };
}

export function areGatewayProfilesEqual(
  left: GatewayProfile,
  right: GatewayProfile | undefined,
): boolean {
  if (!right) {
    return false;
  }

  return (
    JSON.stringify(normalizeGatewayProfileForComparison(left)) ===
    JSON.stringify(normalizeGatewayProfileForComparison(right))
  );
}
