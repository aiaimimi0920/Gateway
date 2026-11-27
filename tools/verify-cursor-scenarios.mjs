// Operator opt-in only: serial real Codex -> Gateway -> exact Cursor account acceptance.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { codingClient } from "./cursor_remote/coding_client.mjs";
import { exactCompletedResponse, exactRoute } from "./cursor_remote/gateway_acceptance.mjs";
import { makeScenario, matrixCases, verifyMultiFileFixture } from "./cursor_remote/scenario_cases.mjs";
import { fixtureEvidence, scenarioEvidence, sha256 } from "./cursor_remote/scenario_evidence.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const arg = (key) => process.argv[process.argv.indexOf(key) + 1];
if (!process.argv.includes("--allow-remote-matrix")) throw new Error("explicit_remote_opt_in_required");
for (const key of ["--base", "--provider", "--credential", "--codex"]) {
  if (!process.argv.includes(key) || !arg(key)) throw new Error("missing_acceptance_argument");
}
const base = arg("--base"), provider = arg("--provider"), credential = arg("--credential"), codex = arg("--codex");
const auditDatabase = process.argv.includes("--audit-database") ? arg("--audit-database") : null;
if (auditDatabase && !path.isAbsolute(auditDatabase)) throw new Error("absolute_audit_database_required");
const url = new URL(base);
if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username || url.password ||
    url.search || url.hash || url.pathname !== "/") throw new Error("isolated_loopback_required");
if (!path.isAbsolute(codex)) throw new Error("absolute_codex_required");
const version = spawnSync(codex, ["--version"], { encoding: "utf8", timeout: 5000, windowsHide: true });
if (version.status !== 0 || version.stdout.trim() !== "codex-cli 0.155.1") throw new Error("wrong_codex_version");
const managementToken = process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
delete process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
if (!managementToken) throw new Error("management_token_required");
const id = randomUUID(), model = "cursor-personal";
const evidence = path.join(root, "target/provider-evidence", `cursor-scenarios-${id}.json`);
const result = { id, startedAt: new Date().toISOString(), base, provider, credential, model,
  codexVersion: version.stdout.trim(), cases: [], localCursorRuntime: false, actualModel: null,
  freeAttribution: "not_proven_by_protocol", retriesEnabled: false,
  auditSource: auditDatabase ? "sqlite-read-only" : "management-api" };
async function api(endpoint, body) {
  const response = await fetch(base.replace(/\/$/, "") + endpoint, {
    method: body === undefined ? "GET" : "POST", redirect: "error",
    headers: { "x-management-token": managementToken, authorization: `Bearer ${managementToken}`,
      "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`management_http_${response.status}`);
  return response.json();
}
async function configuration() {
  const value = (await api("/v1/internal/gateway/console/route-config")).routeConfig;
  return { ...exactRoute(value, provider, credential), documentSha256: sha256(JSON.stringify(value.document)) };
}
const errorCode = (error) => error.name === "TimeoutError" ? "management_timeout" :
  /^[a-z0-9_]+$/.test(error.message) ? error.message : "acceptance_failed";
async function readAudits(key) {
  if (!auditDatabase) return (await api(
    `/v1/internal/gateway/requests?accessKeyId=${encodeURIComponent(key)}&limit=100`)).requests;
  const read = spawnSync("python", ["-B", path.join(root, "tools/cursor_remote/read_scenario_audits.py"),
    auditDatabase, key], { encoding: "utf8", timeout: 15000, maxBuffer: 256 * 1024, windowsHide: true });
  if (read.status !== 0 || read.error) throw new Error("readonly_audit_failed");
  return JSON.parse(read.stdout).requests;
}
function save() {
  fs.mkdirSync(path.dirname(evidence), { recursive: true });
  fs.writeFileSync(evidence, JSON.stringify(result, null, 2) + "\n", "utf8");
}
function safeAudit(a) {
  return { id: a.id, accessKeyId: a.accessKeyId, endpointKind: a.endpointKind, status: a.status,
    providerAccountId: a.providerAccountId, upstreamStatus: a.upstreamStatus,
    createdAt: a.createdAt, completedAt: a.completedAt,
    routeTrace: Object.fromEntries(["routeRevision", "routeAttemptCount", "realCredentialRef",
      "selectedPipelineMode", "errorCode", "fallbackEligible", "cursorRemoteDeliveryUnknown"]
      .filter((k) => a.routeTrace?.[k] !== undefined).map((k) => [k, a.routeTrace[k]])) };
}
async function runCase(name) {
  const scenario = makeScenario(name), workspace = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi",
    `cursor-scenario-${id}-${name}`);
  const item = { name, workspace, startedAt: new Date().toISOString(), expectedTools: scenario.steps.length,
    localKeyRevoked: false, passed: false };
  result.cases.push(item);
  let key;
  try {
    item.before = await configuration();
    if (JSON.stringify(item.before) !== JSON.stringify(result.before)) throw new Error("configuration_changed");
    fs.mkdirSync(workspace);
    for (const [file, content] of Object.entries(scenario.files)) {
      fs.writeFileSync(path.join(workspace, file), content, { encoding: "utf8", flag: "wx" });
    }
    if (name === "multi-file") {
      item.baseline = verifyMultiFileFixture(false);
      if (!item.baseline.expectedFailures || item.baseline.timedOut) throw new Error("fixture_baseline_invalid");
    }
    key = await api("/v1/internal/gateway/access/keys", {
      ownerType: "user", ownerId: "cursor-scenario-acceptance", resolvedProjectId: "local",
      resolvedTenantId: "local", keyKind: "normal", publicKeyPrefix: "sk-gw",
      displayName: `Cursor scenario ${id} ${name}`, expiresAt: new Date(Date.now() + 300000).toISOString(),
      bundleIds: [], metadata: { scopes: ["responses"], models: [model], providerIds: [provider] },
    });
    item.accessKeyId = key.id;
    if (typeof key.token !== "string" || !key.token) throw new Error("local_key_missing");
    item.preflight = await configuration();
    if (JSON.stringify(item.preflight) !== JSON.stringify(item.before)) throw new Error("route_changed_before_send");
    const cli = await codingClient({ codex, base, workspace, token: key.token, prompt: scenario.prompt });
    key.token = undefined;
    item.cli = { exitCode: cli.exitCode, bounded: cli.bounded, sawReconnect: cli.sawReconnect,
      errorCodes: cli.errorCodes, ...scenarioEvidence(cli.events, scenario) };
    item.fixtures = fixtureEvidence(workspace, scenario.expectedFiles);
    if (name === "multi-file" && item.fixtures.exactFiles && item.fixtures.onlyExpectedFiles) {
      item.independentTests = verifyMultiFileFixture(true);
    }
    let audits;
    const deadline = Date.now() + 10000;
    do {
      audits = await readAudits(key.id);
      if (audits.length >= scenario.steps.length + 1 && audits.every((a) => a.completedAt)) break;
      await new Promise((resolve) => setTimeout(resolve, 300));
    } while (Date.now() < deadline);
    item.audits = audits.map(safeAudit);
    item.after = await configuration();
    item.passed = cli.exitCode === 0 && !cli.bounded && !cli.sawReconnect && !cli.errorCodes.length &&
      item.cli.orderedTools && item.cli.exactFinalReply && item.cli.finalReplyAfterTools &&
      item.cli.turnCompletedOnce && !item.cli.errors.length && item.fixtures.onlyExpectedFiles &&
      item.fixtures.exactFiles && JSON.stringify(item.before) === JSON.stringify(item.after) &&
      (name !== "multi-file" || item.independentTests?.passed === true) &&
      item.audits.length === scenario.steps.length + 1 &&
      item.audits.every((a) => a.accessKeyId === key.id && exactCompletedResponse(a, item.preflight));
  } catch (error) { item.error = errorCode(error); }
  finally {
    if (key?.id) {
      try {
        await api(`/v1/internal/gateway/access/keys/${encodeURIComponent(key.id)}/revoke`,
          { reason: "cursor_scenario_acceptance_finished" });
        item.localKeyRevoked = (await api("/v1/internal/gateway/access/catalog"))
          .accessKeys.find((k) => k.id === key.id)?.status === "revoked";
      } catch { item.cleanupError = "test_key_revoke_unconfirmed"; }
      key.token = undefined;
    }
    item.finishedAt = new Date().toISOString();
    item.passed = item.passed && item.localKeyRevoked;
    save();
    console.log(JSON.stringify({ name, passed: item.passed, cli: item.cli, error: item.error,
      auditCount: item.audits?.length, localKeyRevoked: item.localKeyRevoked, evidence }));
  }
  return item.passed;
}
try {
  result.before = await configuration();
  for (const name of matrixCases) {
    if (!await runCase(name)) break; // A failed case is evidence, never silently replay it.
  }
  result.after = await configuration();
  result.passed = result.cases.length === matrixCases.length && result.cases.every((c) => c.passed) &&
    JSON.stringify(result.before) === JSON.stringify(result.after);
} catch (error) { result.error = errorCode(error); result.passed = false; }
finally {
  result.finishedAt = new Date().toISOString();
  save();
  console.log(JSON.stringify({ evidence, passed: result.passed, cases: result.cases.map((c) =>
    ({ name: c.name, passed: c.passed, toolCalls: c.cli?.commands.length, auditCount: c.audits?.length })) }));
  if (!result.passed) process.exitCode = 1;
}
