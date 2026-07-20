import type {
  GatewayApiTestResult,
  GatewayHealthResponse,
  GatewayHttpProbe,
  GatewayLogTail,
  GatewayModelListResponse,
  GatewayProcessSnapshot,
  GatewayProfile,
  GatewayReadyResponse,
  GatewayUiRuntimeInfo,
} from "./types";

const SENSITIVE_ENV_KEY_PATTERN = /(TOKEN|SECRET|PASSWORD|PASS|KEY|AUTH|CREDENTIAL|COOKIE)/i;
const REDACTED = "[redacted]";
const DIAGNOSTIC_VALUE_MAX_CHARS = 1200;
const DIAGNOSTIC_LOG_LINE_MAX_CHARS = 800;
const DIAGNOSTIC_LOG_LINE_LIMIT = 80;

type GatewayDiagnosticsInput = {
  runtimeInfo?: GatewayUiRuntimeInfo;
  selectedProfileName: string;
  draftProfile: GatewayProfile;
  processSnapshot: GatewayProcessSnapshot;
  healthProbe?: GatewayHttpProbe<GatewayHealthResponse>;
  readyProbe?: GatewayHttpProbe<GatewayReadyResponse>;
  modelsProbe?: GatewayHttpProbe<GatewayModelListResponse>;
  apiTestResult?: GatewayApiTestResult;
  logTail: GatewayLogTail;
  baseUrl: string;
  isTauriAvailable: boolean;
};

function maskUrlCredentials(value: string): string {
  try {
    const parsed = new URL(value);
    if (parsed.username || parsed.password) {
      parsed.username = "***";
      parsed.password = "***";
    }
    return parsed.toString();
  } catch {
    return REDACTED;
  }
}

function scrubDiagnosticText(value: string): string {
  let scrubbed = value.replace(
    /((?:token|secret|password|passwd|api[_-]?key|authorization|cookie|credential)\s*[:=]\s*)([^\s,;"']+)/gi,
    "$1[redacted]",
  );
  scrubbed = scrubbed.replace(/(Bearer\s+)[^\s,;"']+/gi, "$1[redacted]");
  scrubbed = scrubbed.replace(
    /(https?:\/\/)([^/\s:@]+):([^/@\s]+)@/gi,
    "$1***:***@",
  );
  return scrubbed;
}

function maskSensitiveValue(key: string, value?: string | null): string {
  const normalizedValue = value?.trim() ?? "";
  if (normalizedValue.length === 0) {
    return "";
  }
  if (SENSITIVE_ENV_KEY_PATTERN.test(key)) {
    return REDACTED;
  }
  if (key === "GATEWAY_REDIS_URL" || key === "GATEWAY_DATABASE_URL" || key === "DATABASE_URL") {
    return maskUrlCredentials(normalizedValue);
  }
  return normalizedValue;
}

function truncateDiagnosticValue(value: string, maxChars = DIAGNOSTIC_VALUE_MAX_CHARS): string {
  if (value.length <= maxChars) {
    return value;
  }
  const omittedChars = value.length - maxChars;
  return `${value.slice(0, maxChars)}...[truncated ${omittedChars} chars]`;
}

function formatProbe<TData>(label: string, probe?: GatewayHttpProbe<TData>): string[] {
  if (!probe) {
    return [`${label}: not checked`];
  }
  const lines = [
    `${label}.ok: ${probe.ok}`,
    `${label}.status: ${probe.status}`,
    `${label}.durationMs: ${probe.durationMs}`,
  ];
  if (probe.error) {
    lines.push(`${label}.error: ${truncateDiagnosticValue(scrubDiagnosticText(probe.error))}`);
  }
  return lines;
}

function formatModelProbe(probe?: GatewayHttpProbe<GatewayModelListResponse>): string[] {
  const lines = formatProbe("models", probe);
  if (probe?.data?.data) {
    lines.push(`models.count: ${probe.data.data.length}`);
  }
  return lines;
}

function formatApiTestResult(apiTestResult?: GatewayApiTestResult): string[] {
  if (!apiTestResult) {
    return ["apiTest: not run"];
  }
  const lines = [
    `apiTest.ok: ${apiTestResult.ok}`,
    `apiTest.status: ${apiTestResult.status}`,
    `apiTest.durationMs: ${apiTestResult.durationMs}`,
    `apiTest.profileName: ${apiTestResult.profileName}`,
    `apiTest.baseUrl: ${maskUrlCredentials(apiTestResult.baseUrl)}`,
    `apiTest.model: ${scrubDiagnosticText(apiTestResult.model)}`,
    `apiTest.requestedAt: ${apiTestResult.requestedAt}`,
    `apiTest.endpoint: ${maskUrlCredentials(apiTestResult.endpoint)}`,
  ];
  if (apiTestResult.error) {
    lines.push(
      `apiTest.error: ${truncateDiagnosticValue(scrubDiagnosticText(apiTestResult.error))}`,
    );
  }
  return lines;
}

function formatRecentLogTail(lines: string[]): string[] {
  if (lines.length === 0) {
    return ["(no recent log tail)"];
  }

  const visibleLines = lines.slice(-DIAGNOSTIC_LOG_LINE_LIMIT).map((line) =>
    truncateDiagnosticValue(scrubDiagnosticText(line), DIAGNOSTIC_LOG_LINE_MAX_CHARS),
  );
  if (lines.length > DIAGNOSTIC_LOG_LINE_LIMIT) {
    return [
      `(showing last ${DIAGNOSTIC_LOG_LINE_LIMIT} of ${lines.length} recent log lines)`,
      ...visibleLines,
    ];
  }
  return visibleLines;
}

function section(title: string, lines: string[]): string[] {
  return [`## ${title}`, ...lines, ""];
}

export function buildGatewayDiagnosticsReport(input: GatewayDiagnosticsInput): string {
  const profile = input.draftProfile;
  const snapshot = input.processSnapshot;
  const extraEnvLines =
    profile.extraEnv.length === 0
      ? ["extraEnv: none"]
      : profile.extraEnv.map((entry) => {
          const key = entry.key.trim() || "(empty-key)";
          return `extraEnv.${key}: ${truncateDiagnosticValue(maskSensitiveValue(key, entry.value))}`;
        });
  const logLines = formatRecentLogTail(input.logTail.lines);

  return [
    "# Neuro Gateway desktop diagnostics",
    "",
    ...section("runtime", [
      `appName: ${input.runtimeInfo?.appName ?? "Neuro Gateway"}`,
      `themeFamily: ${input.runtimeInfo?.themeFamily ?? "NeuroTerminal"}`,
      `gatewayMode: ${input.runtimeInfo?.gatewayMode ?? "headless-first"}`,
      `tauriRuntime: ${input.isTauriAvailable}`,
      `baseUrl: ${maskUrlCredentials(input.baseUrl)}`,
      `selectedProfile: ${input.selectedProfileName}`,
    ]),
    ...section("profile", [
      `name: ${profile.name}`,
      `runtimeRole: ${profile.runtimeRole}`,
      `GATEWAY_MANAGEMENT_TOKEN: ${maskSensitiveValue("GATEWAY_MANAGEMENT_TOKEN", profile.gatewayManagementToken)}`,
      `PORT: ${profile.port}`,
      `GATEWAY_REDIS_URL: ${maskSensitiveValue("GATEWAY_REDIS_URL", profile.gatewayRedisUrl)}`,
      `GATEWAY_DATABASE_URL: ${maskSensitiveValue(
        "GATEWAY_DATABASE_URL",
        profile.gatewayDatabaseUrl,
      )}`,
      `GATEWAY_ROUTES_FILE: ${profile.gatewayRoutesFile?.trim() ?? ""}`,
      `RUST_LOG: ${profile.logLevel?.trim() ?? ""}`,
      `workingDirectory: ${profile.workingDirectory?.trim() ?? ""}`,
      ...extraEnvLines,
    ]),
    ...section("sidecar", [
      `running: ${snapshot.running}`,
      `pid: ${snapshot.pid ?? ""}`,
      `port: ${snapshot.port ?? ""}`,
      `profileName: ${snapshot.profileName ?? ""}`,
      `startupState: ${snapshot.startupState ?? ""}`,
      `shutdownState: ${snapshot.shutdownState ?? ""}`,
      `lastError: ${truncateDiagnosticValue(scrubDiagnosticText(snapshot.lastError ?? ""))}`,
      `logPath: ${snapshot.logPath ?? ""}`,
      `startedAt: ${snapshot.startedAt ?? ""}`,
    ]),
    ...section("probes", [
      ...formatProbe("health", input.healthProbe),
      ...formatProbe("ready", input.readyProbe),
      ...formatModelProbe(input.modelsProbe),
    ]),
    ...section("api test", formatApiTestResult(input.apiTestResult)),
    ...section("recent log tail", logLines),
  ].join("\n");
}
