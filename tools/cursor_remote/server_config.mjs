// Diagnostic projection of pinned SDK/CLI descriptors. Never an endpoint selector.
import { connectJsonHeaders, endpoints, parseJson, ProbeError, request } from "./transport.mjs";

const http2Names = ["UNSPECIFIED", "FORCE_ALL_DISABLED", "FORCE_ALL_ENABLED",
  "FORCE_BIDI_DISABLED", "FORCE_BIDI_ENABLED"];
const updateLevels = ["UNSPECIFIED", "NONE", "RECOMMENDED", "REQUIRED"];

function field(value, name) {
  const snake = name.replace(/[A-Z]/g, (letter) => `_${letter.toLowerCase()}`);
  if (snake !== name && Object.hasOwn(value, name) && Object.hasOwn(value, snake)) {
    throw new ProbeError("invalid_server_config");
  }
  return Object.hasOwn(value, name) ? value[name] : value[snake];
}

function routeClass(value) {
  if (value === undefined || value === "") return "absent";
  if (typeof value !== "string" || value.length > 2048) return "invalid";
  // Exact origins only, not arbitrary returned domains, userinfo, ports, paths or redirects.
  if (["https://agentn.global.api5.cursor.sh", "https://agentn.global.api5.cursor.sh/"].includes(value)) {
    return "current_run_origin";
  }
  if (["https://api2.cursor.sh", "https://api2.cursor.sh/"].includes(value)) return "api_origin";
  return "unapproved_origin";
}

function textPresent(value, name, maxBytes) {
  const raw = field(value, name);
  if (raw === undefined) return false;
  if (typeof raw !== "string" || Buffer.byteLength(raw) > maxBytes) {
    throw new ProbeError("invalid_server_config");
  }
  // Presence only: version and message strings are not trusted printable metadata.
  return raw.length > 0;
}

export function safeClientVersion(value) {
  // Only short numeric release versions are printable; identifiers, prerelease
  // text, URLs and arbitrary server strings remain redacted.
  return typeof value === "string" && /^(?:0|[1-9]\d{0,3})\.(?:0|[1-9]\d{0,3})\.(?:0|[1-9]\d{0,3})(?![\s\S])/.test(value)
    ? value : "unrecognized";
}

function versionValue(value, name) {
  return textPresent(value ?? {}, name, 256) ? safeClientVersion(field(value, name)) : null;
}

function clientVersionSummary(value) {
  if (value !== undefined && (!value || typeof value !== "object" || Array.isArray(value))) {
    throw new ProbeError("invalid_server_config");
  }
  const raw = field(value ?? {}, "updateLevel");
  const level = raw === undefined ? "UNSPECIFIED" : typeof raw === "number"
    ? updateLevels[raw] : typeof raw === "string" ? raw.replace(/^CLIENT_UPDATE_LEVEL_/, "") : null;
  if (!updateLevels.includes(level)) throw new ProbeError("invalid_server_config");
  return {
    clientVersionStatusPresent: value !== undefined,
    clientUpdateLevel: level,
    // Missing/unspecified is unknown, not a server confirmation of compatibility.
    clientUpdateRequired: level === "UNSPECIFIED" ? null : level === "REQUIRED",
    currentClientVersionPresent: textPresent(value ?? {}, "currentClientVersion", 256),
    minSupportedClientVersionPresent: textPresent(value ?? {}, "minSupportedClientVersion", 256),
    minAllowedClientVersionPresent: textPresent(value ?? {}, "minAllowedClientVersion", 256),
    currentClientVersion: versionValue(value, "currentClientVersion"),
    minSupportedClientVersion: versionValue(value, "minSupportedClientVersion"),
    minAllowedClientVersion: versionValue(value, "minAllowedClientVersion"),
    clientVersionMessagePresent: textPresent(value ?? {}, "message", 4096),
  };
}

export function serverConfigSummary(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new ProbeError("invalid_server_config");
  const raw = field(value, "http2Config");
  const name = raw === undefined ? "UNSPECIFIED" : typeof raw === "number"
    ? http2Names[raw] : typeof raw === "string" ? raw.replace(/^HTTP2_CONFIG_/, "") : null;
  if (!http2Names.includes(name)) throw new ProbeError("invalid_server_config");
  const routes = field(value, "agentUrlConfig");
  if (routes !== undefined && (!routes || typeof routes !== "object" || Array.isArray(routes))) {
    throw new ProbeError("invalid_server_config");
  }
  // Other response fields can contain private keys: ignore, never serialize, hash or log them.
  return {
    http2Config: name, configVersionPresent: typeof field(value, "configVersion") === "string",
    agentUrlConfigPresent: routes !== undefined,
    agentRoute: routeClass(routes && field(routes, "agentUrl")),
    agentnRoute: routeClass(routes && field(routes, "agentnUrl")),
    localLoopRoute: routeClass(routes && field(routes, "localLoopInferenceUrl")),
    bidiExplicitlyDisabled: ["FORCE_ALL_DISABLED", "FORCE_BIDI_DISABLED"].includes(name),
    routeSelectionImplemented: false,
    ...clientVersionSummary(field(value, "clientVersionStatus")),
  };
}

export async function inspectServerConfig(token, options, call = request) {
  return serverConfigSummary(parseJson(await call(endpoints.serverConfig, {
    ...options, headers: { ...connectJsonHeaders, authorization: `Bearer ${token}` }, body: "{}",
  })));
}
