import type { GatewayEnvEntry, GatewayProfile, GatewayProfileTransferPayload } from "./types";

const SENSITIVE_ENV_KEY_PATTERN = /(TOKEN|SECRET|PASSWORD|PASS|KEY|AUTH|CREDENTIAL|COOKIE)/i;
const REDACTED_PROFILE_VALUE = "";

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown, fallback = ""): string {
  return typeof value === "string" ? value : fallback;
}

function optionalStringValue(value: unknown): string | null {
  return typeof value === "string" ? value : null;
}

function numberValue(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? Math.trunc(value) : fallback;
}

function sanitizeUrlCredentials(value?: string | null): string {
  const normalizedValue = value?.trim() ?? "";
  if (normalizedValue.length === 0) {
    return "";
  }

  try {
    const parsed = new URL(normalizedValue);
    if (parsed.username || parsed.password) {
      parsed.username = "";
      parsed.password = "";
    }
    return parsed.toString();
  } catch {
    return REDACTED_PROFILE_VALUE;
  }
}

function sanitizeEnvEntry(entry: GatewayEnvEntry): GatewayEnvEntry {
  const key = entry.key.trim();
  return {
    key: entry.key,
    value: SENSITIVE_ENV_KEY_PATTERN.test(key) ? REDACTED_PROFILE_VALUE : entry.value,
  };
}

function sanitizeProfileForExport(profile: GatewayProfile): GatewayProfile {
  return {
    ...profile,
    gatewayManagementToken: REDACTED_PROFILE_VALUE,
    gatewayRedisUrl: sanitizeUrlCredentials(profile.gatewayRedisUrl),
    gatewayDatabaseUrl: sanitizeUrlCredentials(profile.gatewayDatabaseUrl),
    extraEnv: profile.extraEnv.map(sanitizeEnvEntry),
  };
}

function parseEnvEntries(value: unknown): GatewayEnvEntry[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const entries: GatewayEnvEntry[] = [];
  for (const item of value) {
    if (!isRecord(item)) {
      continue;
    }
    entries.push({
      key: stringValue(item.key),
      value: stringValue(item.value),
    });
  }
  return entries;
}

function parseGatewayProfile(value: unknown): GatewayProfile {
  if (!isRecord(value)) {
    throw new Error("导入内容必须包含 Gateway profile 对象。");
  }

  return {
    name: stringValue(value.name, "imported-profile"),
    runtimeRole:
      value.runtimeRole === "worker" || value.runtimeRole === "standalone"
        ? value.runtimeRole
        : "splitter",
    gatewayManagementToken: optionalStringValue(value.gatewayManagementToken),
    port: numberValue(value.port, 4200),
    gatewayRedisUrl: stringValue(value.gatewayRedisUrl),
    gatewayDatabaseUrl: optionalStringValue(value.gatewayDatabaseUrl),
    gatewayRoutesFile: optionalStringValue(value.gatewayRoutesFile),
    logLevel: optionalStringValue(value.logLevel),
    workingDirectory: optionalStringValue(value.workingDirectory),
    extraEnv: parseEnvEntries(value.extraEnv),
  };
}

export function buildGatewayProfileExportText(profile: GatewayProfile): string {
  const payload: GatewayProfileTransferPayload = {
    schemaVersion: 1,
    kind: "neuro-gateway-ui-profile",
    exportedAt: new Date().toISOString(),
    profile: sanitizeProfileForExport(profile),
  };
  return JSON.stringify(payload, null, 2);
}

export function parseGatewayProfileTransferText(text: string): GatewayProfile {
  const trimmed = text.trim();
  if (trimmed.length === 0) {
    throw new Error("请先粘贴 Gateway profile JSON。");
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(trimmed);
  } catch (error) {
    throw new Error(`导入内容不是有效 JSON：${error instanceof Error ? error.message : error}`);
  }

  if (
    isRecord(parsed) &&
    parsed.kind === "neuro-gateway-ui-profile" &&
    parsed.schemaVersion === 1
  ) {
    return parseGatewayProfile(parsed.profile);
  }

  return parseGatewayProfile(parsed);
}
