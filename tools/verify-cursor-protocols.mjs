// Explicit live protocol acceptance. This is not a Codex CLI test or an upstream fixture.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { exactRoute } from "./cursor_remote/gateway_acceptance.mjs";
import { sha256 } from "./cursor_remote/scenario_evidence.mjs";
import { exactProtocolAudit } from "./cursor_remote/protocol_evidence.mjs";
import { protocolCases, protocolScope, runProtocolCase } from "./cursor_remote/protocol_cases.mjs";
import { meteredCases } from "./cursor_remote/metered_protocol_cases.mjs";
import { exactMeteredUsage } from "./cursor_remote/metered_protocol_client.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const arg = (key) => process.argv[process.argv.indexOf(key) + 1];
if (!process.argv.includes("--allow-remote-protocols")) throw new Error("explicit_remote_opt_in_required");
for (const key of ["--base", "--provider", "--credential", "--audit-database"]) {
  if (!process.argv.includes(key) || !arg(key)) throw new Error("missing_acceptance_argument");
}
const base = arg("--base").replace(/\/$/, ""), provider = arg("--provider"), credential = arg("--credential");
const database = arg("--audit-database"), url = new URL(base);
if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username || url.password ||
    url.search || url.hash || url.pathname !== "/" || !path.isAbsolute(database)) throw new Error("local_target_required");
const names = process.argv.includes("--cases") ? arg("--cases").split(",") : protocolCases;
if (!names.length || new Set(names).size !== names.length || names.some((n) => !protocolCases.includes(n))) {
  throw new Error("unknown_or_duplicate_case");
}
const managementToken = process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
delete process.env.CURSOR_ACCEPTANCE_MANAGEMENT_TOKEN;
if (!managementToken) throw new Error("management_token_required");
const id = randomUUID(), evidenceDir = path.join(root, "target/provider-evidence", `cursor-protocols-${id}`);
const evidence = evidenceDir + ".json";
fs.mkdirSync(evidenceDir);
const result = { id, startedAt: new Date().toISOString(), base, provider, credential, cases: [],
  client: "native-http-and-websocket-not-codex", localCursorRuntime: false, retriesEnabled: false,
  actualModel: null, freeAttribution: "not_proven_by_protocol", selectedCases: names };
const errorCode = (error) => /^[a-z0-9_]+$/.test(error.message) ? error.message :
  error.code === "ERR_ASSERTION" ? String(error.message).split("\n")[0].replace(/[^a-z0-9_]/g, "_") :
    error.name === "TimeoutError" ? "request_timeout" : "protocol_acceptance_failed";
async function api(endpoint, body) {
  const response = await fetch(base + endpoint, {
    method: body === undefined ? "GET" : "POST", redirect: "error",
    headers: { "x-management-token": managementToken, authorization: `Bearer ${managementToken}`,
      "content-type": "application/json" }, body: body === undefined ? undefined : JSON.stringify(body),
    signal: AbortSignal.timeout(10000),
  });
  if (!response.ok) throw new Error(`management_http_${response.status}`);
  return response.json();
}
async function configuration() {
  const value = (await api("/v1/internal/gateway/console/route-config")).routeConfig;
  return { ...exactRoute(value, provider, credential), documentSha256: sha256(JSON.stringify(value.document)) };
}
function audits(key) {
  const read = spawnSync("python", ["-B", path.join(root, "tools/cursor_remote/read_scenario_audits.py"), database, key],
    { encoding: "utf8", timeout: 15000, maxBuffer: 256 * 1024, windowsHide: true });
  if (read.status !== 0 || read.error) throw new Error("readonly_audit_failed");
  return JSON.parse(read.stdout).requests;
}
function save() {
  fs.writeFileSync(evidence, JSON.stringify(result, null, 2) + "\n", "utf8");
}
async function runCase(name) {
  const item = { name, startedAt: new Date().toISOString(), passed: false, localKeyRevoked: false };
  result.cases.push(item);
  let key;
  const displayName = `Cursor protocol ${id} ${name}`;
  try {
    item.before = await configuration();
    if (JSON.stringify(item.before) !== JSON.stringify(result.before)) throw new Error("configuration_changed");
    key = await api("/v1/internal/gateway/access/keys", {
      ownerType: "user", ownerId: "cursor-protocol-acceptance", resolvedProjectId: "local",
      resolvedTenantId: "local", keyKind: "normal", publicKeyPrefix: "sk-gw",
      displayName, expiresAt: new Date(Date.now() + 300000).toISOString(),
      bundleIds: [], metadata: { scopes: [protocolScope(name)],
        models: ["cursor-personal"], providerIds: [provider] },
    });
    item.accessKeyId = key.id;
    if (typeof key.token !== "string" || !key.token) throw new Error("local_key_missing");
    await runProtocolCase({ name, root, evidenceDir, base, token: key.token, receipt: item });
    const count = item.phases.length + item.rejections.length, deadline = Date.now() + 10000;
    do {
      item.audits = audits(key.id);
      if (item.audits.length >= count && item.audits.every((a) => a.completedAt)) break;
      await new Promise((resolve) => setTimeout(resolve, 500));
    } while (Date.now() < deadline);
    const positive = item.audits.filter((a) => a.status === "completed");
    const negative = item.audits.filter((a) => a.status !== "completed");
    item.exactPositiveAudit = positive.length === item.phases.length &&
      positive.every((a) => a.accessKeyId === key.id && exactProtocolAudit(a, item.before, item.endpoint) &&
        (!item.protocolFamily || a.routeTrace?.requestedProtocolFamily === item.protocolFamily));
    // Metered cases have one phase and must match the exact account audit counts.
    if (meteredCases.includes(name) && !name.endsWith("-rejections")) {
      item.exactUsageAudit = exactMeteredUsage(item.phases, positive);
      item.exactPositiveAudit &&= item.exactUsageAudit;
    }
    item.zeroSendRejections = negative.length === item.rejections.length && negative.every((a) =>
      a.accessKeyId === key.id && a.endpointKind === item.endpoint && a.completedAt &&
      a.upstreamStatus == null && a.routeTrace?.routeAttemptCount === 0 &&
      (!item.protocolFamily || a.routeTrace?.requestedProtocolFamily === item.protocolFamily) &&
      a.routeTrace?.cursorRemoteDeliveryUnknown === null) &&
      JSON.stringify(negative.map((a) => a.routeTrace?.errorCode).sort()) ===
      JSON.stringify(item.rejections.map((r) => r.code).sort());
    item.after = await configuration();
    item.passed = item.wirePassed && item.exactPositiveAudit && item.zeroSendRejections &&
      JSON.stringify(item.before) === JSON.stringify(item.after);
  } catch (error) { item.error = errorCode(error); }
  finally {
    if (!key?.id) {
      try {
        const matches = (await api("/v1/internal/gateway/access/catalog")).accessKeys
          .filter((k) => k.displayName === displayName && k.ownerId === "cursor-protocol-acceptance");
        if (matches.length === 1) { key = { id: matches[0].id }; item.accessKeyId = key.id; }
        else item.creationCleanup = matches.length === 0 ? "no_matching_key" : "ambiguous_key";
      } catch { item.creationCleanup = "lookup_unconfirmed"; }
    }
    if (key?.id) {
      try {
        await api(`/v1/internal/gateway/access/keys/${encodeURIComponent(key.id)}/revoke`,
          { reason: "cursor_protocol_acceptance_finished" });
        item.localKeyRevoked = (await api("/v1/internal/gateway/access/catalog"))
          .accessKeys.find((k) => k.id === key.id)?.status === "revoked";
      } catch { item.cleanupError = "test_key_cleanup_unconfirmed"; }
      try { item.audits ??= audits(key.id); } catch { item.auditReadError = true; }
      key.token = undefined;
    }
    item.finishedAt = new Date().toISOString();
    item.passed = !!item.passed && item.localKeyRevoked;
    save();
    console.log(JSON.stringify({ name, passed: item.passed, error: item.error, phases: item.phases?.length,
      thinkingObserved: item.thinkingObserved, exactPositiveAudit: item.exactPositiveAudit,
      zeroSendRejections: item.zeroSendRejections, localKeyRevoked: item.localKeyRevoked, evidence }));
  }
  return item.passed;
}
try {
  result.before = await configuration();
  for (const name of names) if (!await runCase(name)) break; // Stop, do not automatically replay a failed case.
  result.after = await configuration();
  result.passed = result.cases.length === names.length && result.cases.every((c) => c.passed) &&
    JSON.stringify(result.before) === JSON.stringify(result.after);
} catch (error) { result.error = errorCode(error); result.passed = false; }
finally {
  result.finishedAt = new Date().toISOString();
  save();
  console.log(JSON.stringify({ evidence, passed: result.passed, cases: result.cases.map((c) =>
    ({ name: c.name, passed: c.passed, error: c.error })) }));
  if (!result.passed) process.exitCode = 1;
}
