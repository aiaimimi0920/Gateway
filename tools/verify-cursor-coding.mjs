// Explicit operator opt-in. Real fixed Codex edits a disposable project through Gateway Responses.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { codingClient } from "./cursor_remote/coding_client.mjs";
import { developmentCommands } from "./cursor_remote/coding_evidence.mjs";
import { originalSource, expectedSource, expectedTests, verifyTests } from "./cursor_remote/coding_fixture.mjs";
import { exactCompletedResponse, exactRoute } from "./cursor_remote/gateway_acceptance.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const arg = (key) => process.argv[process.argv.indexOf(key) + 1];
if (!process.argv.includes("--allow-development-run")) throw new Error("explicit_remote_opt_in_required");
for (const key of ["--base", "--provider", "--credential", "--codex"]) {
  if (!process.argv.includes(key) || !arg(key)) throw new Error("missing_acceptance_argument");
}
const base = arg("--base"), provider = arg("--provider"), credential = arg("--credential");
const url = new URL(base);
if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username || url.password ||
    url.search || url.hash || url.pathname !== "/") throw new Error("isolated_loopback_required");
const managementToken = process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
delete process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
if (!managementToken) throw new Error("management_token_required");
if (!path.isAbsolute(arg("--codex"))) throw new Error("absolute_codex_required");
const version = spawnSync(arg("--codex"), ["--version"], { encoding: "utf8", timeout: 5000, windowsHide: true });
if (version.status !== 0 || version.stdout.trim() !== "codex-cli 0.155.1") throw new Error("wrong_codex_version");
const id = randomUUID(), model = "cursor-personal";
const workspace = path.join("C:/Users/Public/nas_home/AI/GameEditor/linshi", `cursor-development-${id}`);
const evidence = path.join(root, "target/provider-evidence", `cursor-coding-${id}.json`);
const result = { id, observedAt: new Date().toISOString(), base, workspace, model, provider, credential,
  mode: "real-codex-workspace-write", codexVersion: version.stdout.trim(), routeChanged: false, cursorCredentialRead: false,
  localCursorRuntime: false, localKeyRevoked: false, freeAttribution: "not_proven_by_protocol", actualModel: null };
let key;
async function api(endpoint, body) {
  const response = await fetch(base.replace(/\/$/, "") + endpoint, {
    method: body === undefined ? "GET" : "POST", redirect: "error",
    headers: { authorization: `Bearer ${managementToken}`, "x-management-token": managementToken,
      "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body), signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`management_http_${response.status}`);
  return response.json();
}
const route = async () => exactRoute((await api("/v1/internal/gateway/console/route-config")).routeConfig, provider, credential);
function regular(file) {
  const full = path.join(workspace, file), info = fs.lstatSync(full);
  const parent = fs.lstatSync(workspace);
  if (!info.isFile() || info.isSymbolicLink() || info.nlink !== 1 || info.size > 4096 ||
      parent.isSymbolicLink() || fs.realpathSync(full).toLowerCase() !== full.toLowerCase() ||
      fs.realpathSync(workspace).toLowerCase() !== workspace.toLowerCase()) throw new Error("fixture_path_changed");
  return fs.readFileSync(full);
}
const hash = (file) => createHash("sha256").update(regular(file)).digest("hex");
try {
  result.before = await route();
  fs.mkdirSync(workspace, { recursive: true });
  fs.writeFileSync(path.join(workspace, "calc.mjs"), originalSource, "utf8");
  fs.writeFileSync(path.join(workspace, "calc.test.mjs"), expectedTests, "utf8");
  result.beforeFiles = { source: hash("calc.mjs"), tests: hash("calc.test.mjs") };
  result.baseline = verifyTests(originalSource);
  if (!result.baseline.expectedAssertionFailures || result.baseline.timedOut) throw new Error("fixture_baseline_invalid");
  key = await api("/v1/internal/gateway/access/keys", {
    ownerType: "user", ownerId: "cursor-development-acceptance", resolvedProjectId: "local",
    resolvedTenantId: "local", keyKind: "normal", publicKeyPrefix: "sk-gw",
    displayName: `Cursor coding ${id}`, expiresAt: new Date(Date.now() + 300000).toISOString(),
    bundleIds: [], metadata: { scopes: ["responses"], models: [model], providerIds: [provider] },
  });
  if (typeof key.token !== "string" || !key.token) throw new Error("local_key_missing");
  result.accessKeyId = key.id;
  result.preflight = await route();
  if (result.preflight.revision !== result.before.revision) throw new Error("route_changed_before_send");
  const cli = await codingClient({ codex: arg("--codex"), base, workspace, token: key.token,
    prompt: "Fix the add function in calc.mjs. Use the declared exec_command function for local operations. " +
      "First read calc.mjs and calc.test.mjs, then in a separate tool call edit ONLY calc.mjs, " +
      "then run node calc.test.mjs and report the actual result. This runs the same node:test assertions " +
      "without spawning a test child process, which the Windows sandbox blocks. Do not use --test, " +
      "disable the sandbox, or request escalation. Do not modify tests or any other file. " +
      "Do not access other directories, use network, ask questions, or invoke Cursor built-in tools. " +
      `The final calc.mjs must be exactly this UTF-8 source without BOM: ${JSON.stringify(expectedSource)}. ` +
      "Use PowerShell commands with login:false. No retries after an error." });
  key.token = undefined;
  const development = developmentCommands(cli.events, originalSource, expectedTests);
  result.cli = { exitCode: cli.exitCode, bounded: cli.bounded, sawReconnect: cli.sawReconnect,
    errorCodes: cli.errorCodes, outputBytes: cli.outputBytes, eventTypes: cli.events.map((e) => e.type),
    ...development,
    finalMessageCount: cli.events.filter((e) => e.type === "item.completed" &&
      e.item?.type === "agent_message" && typeof e.item.text === "string" && e.item.text.trim()).length,
    errors: cli.events.filter((e) => e.type === "error" || e.type === "turn.failed").map((e) => e.type) };
  result.afterFiles = { source: hash("calc.mjs"), tests: hash("calc.test.mjs") };
  // Do not execute arbitrary model-written code outside its client sandbox.
  result.trustedSourceMatched = regular("calc.mjs").toString("utf8") === expectedSource;
  const unchangedTest = regular("calc.test.mjs").toString("utf8") === expectedTests;
  result.independentTests = result.trustedSourceMatched && unchangedTest ? verifyTests(expectedSource) : { passed: false, skipped: true };
  result.projectFiles = fs.readdirSync(workspace).filter((name) => name !== "home").sort();
  let audits;
  const deadline = Date.now() + 10000;
  do {
    audits = await api(`/v1/internal/gateway/requests?accessKeyId=${encodeURIComponent(key.id)}&limit=100`);
    if (audits.requests.length && audits.requests.every((a) => a.completedAt)) break;
    await new Promise((resolve) => setTimeout(resolve, 250));
  } while (Date.now() < deadline);
  result.audits = audits.requests.map((a) => ({ id: a.id, endpointKind: a.endpointKind, status: a.status,
    providerAccountId: a.providerAccountId, upstreamStatus: a.upstreamStatus, responseId: a.responseId,
    createdAt: a.createdAt, completedAt: a.completedAt,
    routeTrace: Object.fromEntries(["routeRevision", "routeAttemptCount", "realCredentialRef",
      "selectedPipelineMode", "errorCode", "fallbackEligible", "cursorRemoteDeliveryUnknown"]
      .filter((k) => a.routeTrace?.[k] !== undefined).map((k) => [k, a.routeTrace[k]])) }));
  result.after = await route();
  result.passed = cli.exitCode === 0 && !cli.bounded && !cli.sawReconnect &&
    development.orderedReadEditTest && development.finalReplyAfterTools &&
    cli.events.filter((e) => e.type === "turn.completed").length === 1 && result.cli.errors.length === 0 && result.cli.finalMessageCount > 0 &&
    result.beforeFiles.source !== result.afterFiles.source && result.beforeFiles.tests === result.afterFiles.tests &&
    result.projectFiles.join(",") === "calc.mjs,calc.test.mjs" && result.independentTests.passed &&
    result.before.revision === result.after.revision && result.audits.length >= 4 &&
    result.audits.every((a) => exactCompletedResponse(a, result.preflight));
} catch (error) {
  result.error = /^[a-z0-9_]+$/.test(error.message) ? error.message : "acceptance_failed";
} finally {
  if (key?.id) {
    try {
      await api(`/v1/internal/gateway/access/keys/${encodeURIComponent(key.id)}/revoke`,
        { reason: "cursor_development_acceptance_finished" });
      result.localKeyRevoked = (await api("/v1/internal/gateway/access/catalog"))
        .accessKeys.find((k) => k.id === key.id)?.status === "revoked";
    } catch { result.cleanupError = "test_key_revoke_unconfirmed"; }
    key.token = undefined;
  }
  result.finishedAt = new Date().toISOString();
  result.passed = result.passed === true && result.localKeyRevoked;
  fs.mkdirSync(path.dirname(evidence), { recursive: true });
  fs.writeFileSync(evidence, JSON.stringify(result, null, 2) + "\n", { encoding: "utf8", flag: "wx" });
  console.log(JSON.stringify({ evidence, ...result }));
  if (!result.passed || !result.localKeyRevoked) process.exitCode = 1;
}
