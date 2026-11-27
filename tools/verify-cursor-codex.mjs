// Explicit opt-in acceptance: real Codex -> real Gateway -> one remote Cursor Run.
// Caller supplies management auth through environment; no Cursor credential is read.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { cursorErrorCodes, exactCompletedResponse, exactRoute } from "./cursor_remote/gateway_acceptance.mjs";
import { httpOnce } from "./cursor_remote/gateway_http_acceptance.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const arg = (key) => process.argv[process.argv.indexOf(key) + 1];
if (!process.argv.includes("--allow-one-remote-run")) throw new Error("explicit_remote_opt_in_required");
const httpMode = process.argv.includes("--http-one-shot");
for (const name of ["--base", "--provider", "--credential", ...(httpMode ? [] : ["--codex"])]) {
  if (!process.argv.includes(name) || !arg(name)) throw new Error("missing_acceptance_argument");
}
const base = arg("--base"), provider = arg("--provider"), credential = arg("--credential");
const url = new URL(base);
if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username || url.password ||
    url.search || url.hash || url.pathname !== "/") throw new Error("isolated_loopback_required");
const managementToken = process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
delete process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
if (!managementToken) throw new Error("management_token_required");
const id = randomUUID(), model = "cursor-personal", expected = "CURSOR GATEWAY OK - 2 + 3 = 5";
const workspace = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi", `cursor-codex-${id}`);
const evidence = path.join(root, "target/provider-evidence", `cursor-gateway-${httpMode ? "http" : "codex"}-${id}.json`);
const result = { id, observedAt: new Date().toISOString(), gateway: base, model,
  mode: httpMode ? "http-one-shot" : "codex-cli",
  routeChanged: false, cursorCredentialRead: false, localKeyRevoked: false,
  freeAttribution: "not_proven_by_protocol", actualModel: null };
let key;
async function api(endpoint, body) {
  const response = await fetch(base.replace(/\/$/, "") + endpoint, {
    method: body === undefined ? "GET" : "POST",
    redirect: "error",
    headers: { authorization: `Bearer ${managementToken}`, "x-management-token": managementToken,
      "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`management_http_${response.status}`);
  return response.json();
}
function route(value) {
  return exactRoute(value, provider, credential);
}
async function codex(token) {
  const settings = {
    model_provider: "cursor_acceptance", model, approval_policy: "never", project_doc_max_bytes: 0,
    check_for_update_on_startup: false, web_search: "disabled",
    model_reasoning_effort: "none", model_reasoning_summary: "none",
    "tools.experimental_request_user_input.enabled": false,
    "features.shell_tool": false, "features.unified_exec": false, "features.apply_patch_freeform": false,
    "features.view_image": false, "features.apps": false, "features.plugins": false, "features.collab": false,
    "features.default_mode_request_user_input": false, "features.collaboration_modes": false,
    "features.remote_models": false, "features.api_key_model_discovery": false,
    "features.responses_websockets": false, "features.responses_websockets_v2": false,
    "model_providers.cursor_acceptance.name": "Cursor Gateway isolated acceptance",
    "model_providers.cursor_acceptance.base_url": `${base.replace(/\/$/, "")}/v1`,
    "model_providers.cursor_acceptance.wire_api": "responses",
    "model_providers.cursor_acceptance.env_key": "CURSOR_ACCEPTANCE_LOCAL_KEY",
    "model_providers.cursor_acceptance.requires_openai_auth": false,
    "model_providers.cursor_acceptance.supports_websockets": false,
    "model_providers.cursor_acceptance.request_max_retries": 0,
    "model_providers.cursor_acceptance.stream_max_retries": 0,
    "model_providers.cursor_acceptance.stream_idle_timeout_ms": 130000,
  };
  const args = ["exec", "--ignore-user-config", "--ignore-rules", "--ephemeral", "--skip-git-repo-check",
    "--sandbox", "read-only", "--color", "never", "--json", "--cd", workspace];
  for (const [name, value] of Object.entries(settings)) args.push("-c", `${name}=${JSON.stringify(value)}`);
  args.push(`This is a single text connectivity test. Do not use tools, read files, or retry. Reply exactly: ${expected}`);
  const env = {};
  for (const name of ["SystemRoot", "WINDIR", "PATH", "PATHEXT", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "ComSpec"]) {
    const actual = Object.keys(process.env).find((k) => k.toLowerCase() === name.toLowerCase());
    if (actual) env[actual] = process.env[actual];
  }
  Object.assign(env, { CODEX_HOME: path.join(workspace, "home"), TEMP: workspace, TMP: workspace,
    CURSOR_ACCEPTANCE_LOCAL_KEY: token, NO_PROXY: "127.0.0.1,localhost" });
  return new Promise((resolve, reject) => {
    const child = spawn(arg("--codex"), args, { env, cwd: workspace, windowsHide: true,
      stdio: ["ignore", "pipe", "pipe"] });
    delete env.CURSOR_ACCEPTANCE_LOCAL_KEY;
    let output = "", size = 0, bounded = false;
    const timer = setTimeout(() => { bounded = true; child.kill(); }, 145000);
    for (const stream of [child.stdout, child.stderr]) stream.on("data", (bytes) => {
      size += bytes.length;
      if (size > 512 * 1024) { bounded = true; child.kill(); return; }
      output += bytes.toString("utf8");
    });
    child.once("error", () => { clearTimeout(timer); reject(new Error("codex_spawn_failed")); });
    child.once("close", (exitCode) => {
      clearTimeout(timer);
      const events = [];
      for (const line of output.split(/\r?\n/)) {
        try { const event = JSON.parse(line); if (typeof event.type === "string") events.push(event); } catch {}
      }
      const messages = events.filter((e) => e.type === "item.completed" && e.item?.type === "agent_message");
      resolve({ exitCode, bounded, outputBytes: size, eventTypes: events.map((e) => e.type),
        expectedReplyObserved: messages.some((e) => e.item.text?.trim() === expected),
        sawReconnect: /Reconnecting|retrying/.test(output),
        errorCodes: cursorErrorCodes(output) });
    });
  });
}
try {
  result.before = route((await api("/v1/internal/gateway/console/route-config")).routeConfig);
  fs.mkdirSync(path.join(workspace, "home"), { recursive: true });
  key = await api("/v1/internal/gateway/access/keys", {
    ownerType: "user", ownerId: "cursor-connectivity-acceptance", resolvedProjectId: "local",
    resolvedTenantId: "local", keyKind: "normal", publicKeyPrefix: "sk-gw",
    displayName: `Cursor Codex one-shot ${id}`, expiresAt: new Date(Date.now() + 300000).toISOString(),
    bundleIds: [], metadata: { scopes: ["responses"], models: [model], providerIds: [provider] },
  });
  if (typeof key.token !== "string" || !key.token) throw new Error("local_key_missing");
  result.accessKeyId = key.id;
  result.preflight = route((await api("/v1/internal/gateway/console/route-config")).routeConfig);
  if (result.preflight.revision !== result.before.revision) throw new Error("route_changed_before_send");
  // The provider-scoped key is not a credential pin: shared-instance acceptance
  // requires a no-concurrent-route-edit window, then verifies the actual audit.
  try {
    if (httpMode) result.http = await httpOnce(base, key.token, expected);
    else result.cli = await codex(key.token);
  } catch { result.requestError = "acceptance_request_failed_no_retry"; }
  key.token = undefined;
  let audits;
  const auditDeadline = Date.now() + 10000;
  do {
    audits = await api(`/v1/internal/gateway/requests?accessKeyId=${encodeURIComponent(key.id)}&limit=10`);
    if (audits.requests.length > 0 && audits.requests.every((a) => a.completedAt)) break;
    await new Promise((resolve) => setTimeout(resolve, 250));
  } while (Date.now() < auditDeadline);
  result.audits = audits.requests.map((a) => ({ id: a.id, endpointKind: a.endpointKind, status: a.status,
    providerAccountId: a.providerAccountId, upstreamStatus: a.upstreamStatus, responseId: a.responseId,
    createdAt: a.createdAt, completedAt: a.completedAt,
    routeTrace: Object.fromEntries(["routeRevision", "routeAttemptCount", "realCredentialRef",
      "selectedPipelineMode", "errorCode", "fallbackEligible", "cursorRemoteDeliveryUnknown"]
      .filter((k) => a.routeTrace?.[k] !== undefined).map((k) => [k, a.routeTrace[k]])) }));
  result.after = route((await api("/v1/internal/gateway/console/route-config")).routeConfig);
  const replyPassed = httpMode
    ? result.http?.httpStatus === 200 && result.http.completed && result.http.expectedReplyObserved
    : result.cli?.exitCode === 0 && !result.cli.bounded && result.cli.expectedReplyObserved && !result.cli.sawReconnect;
  result.passed = !!replyPassed &&
    result.before.revision === result.after.revision && result.audits.length === 1 &&
    exactCompletedResponse(result.audits[0], result.preflight);
} catch (error) {
  result.error = /^[a-z0-9_]+$/.test(error.message) ? error.message : "acceptance_failed";
} finally {
  if (key?.id) {
    try {
      await api(`/v1/internal/gateway/access/keys/${encodeURIComponent(key.id)}/revoke`,
        { reason: "cursor_one_shot_acceptance_finished" });
      result.localKeyRevoked = (await api("/v1/internal/gateway/access/catalog"))
        .accessKeys.find((k) => k.id === key.id)?.status === "revoked";
    } catch { result.cleanupError = "test_key_revoke_unconfirmed"; }
    key.token = undefined;
  }
  result.finishedAt = new Date().toISOString();
  result.passed = result.passed === true && result.localKeyRevoked === true;
  fs.mkdirSync(path.dirname(evidence), { recursive: true });
  fs.writeFileSync(evidence, JSON.stringify(result, null, 2) + "\n", { encoding: "utf8", flag: "wx" });
  console.log(JSON.stringify({ evidence, ...result }));
  if (!result.passed || !result.localKeyRevoked) process.exitCode = 1;
}
