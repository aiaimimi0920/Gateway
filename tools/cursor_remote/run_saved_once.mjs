// Explicit operator-only experiment, not a Gateway runtime or a production route.
// Reuses one imported credential without login/refresh; exactly one Run maximum.
import { randomUUID } from "node:crypto";
import { pathToFileURL } from "node:url";
import { savedArguments } from "./inspect_saved.mjs";
import { openSavedCredential } from "./saved_credential.mjs";
import { models, verifyIdentity } from "./qualification.mjs";
import { inspectServerConfig } from "./server_config.mjs";
import { inspectModelSelection } from "./model_selection.mjs";
import { inferOnce } from "./inference.mjs";
import { createProbeReporter, projectReceipt } from "./receipt.mjs";
import { endpoints, ProbeError, request, settledRequests } from "./transport.mjs";

const reads = new Set(["me", "usage", "grants", "stripe", "teams", "hardLimit", "models",
  "defaultModel", "serverConfig", "cliModels", "cliDefaultModel", "parameterModels"].map((key) => endpoints[key]));
// Operator-only protocol experiment, pinned to a statically inspected official archive.
// This announces a compatibility surface, not execution of the official CLI.
export const cliCompatibilityExperiment = "cli_20261122_e9042b2";
const cliHeaders = Object.freeze({
  "x-cursor-client-type": "cli", "x-cursor-client-version": "cli-2026.11.22-e9042b2",
});
const safeError = (error) => ({ code: error instanceof ProbeError ? error.code : "probe_failed",
  ...(error instanceof ProbeError ? { operation: error.operation, jsonReason: error.jsonReason,
    transportPhase: error.transportPhase, transportCode: error.transportCode } : {}),
  ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) });

function requireSavedServerConfig(config, compatibilityExperiment) {
  // Current official CLI headers also get the numeric editor-version REQUIRED result.
  // Only this explicit pinned experiment treats it as diagnostic, never as proof of
  // Run compatibility. Default behavior and all billing/routing gates stay closed.
  if (config.clientUpdateRequired === true && !compatibilityExperiment) {
    throw new ProbeError("saved_run_client_update_required");
  }
  // Only the already reviewed fixed origin is admitted. Do not guess routing when
  // privacy variants disagree, or silently ignore a server HTTP/2-disable instruction.
  if (config.bidiExplicitlyDisabled || !config.agentUrlConfigPresent ||
      [config.agentRoute, config.agentnRoute, config.localLoopRoute].some((route) => route !== "current_run_origin")) {
    throw new ProbeError("saved_run_route_unconfirmed");
  }
}

function requireSavedModelSelection(selection) {
  if (!selection.cliSelectorSafe || !selection.sameDefaultSelector || selection.defaultNeedsMapping ||
      selection.defaultNameMatches !== 1 || selection.defaultParameterDefinitions !== 0 ||
      selection.defaultVariants !== 1 || selection.defaultVariantParameterValues !== 0 ||
      selection.defaultVariantMaxMode !== false || selection.defaultSupportsAgent === false ||
      selection.defaultRequiresDataRetention === true) {
    throw new ProbeError("saved_run_selection_unconfirmed");
  }
}

export async function runSavedOnce(argv, { call = request, open = openSavedCredential,
  reporter = createProbeReporter, compatibilityExperiment } = {}) {
  if (compatibilityExperiment !== undefined && compatibilityExperiment !== cliCompatibilityExperiment) {
    throw new ProbeError("invalid_client_compatibility_experiment");
  }
  const args = savedArguments(argv), controller = new AbortController();
  const receipt = reporter({ mode: "saved-once", controller, emitProjection: true });
  const log = (kind, data) => receipt.log(kind, data);
  const stop = () => controller.abort();
  // Preflight + one bounded Run + postflight; no daemon or retained session.
  const timeout = setTimeout(stop, 120000);
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  let saved, failure, runAttempted = false;
  try {
    saved = open(args);
    log("saved_credential_loaded", { credentialSource: "gateway_active_revision" });
    if (compatibilityExperiment) log("client_compatibility_experiment", {
      clientProfile: compatibilityExperiment, configVersionGate: "diagnostic_only",
      actualCliExecuted: false, attemptLimit: 1, fallback: false,
    });
    const checkedCall = async (url, options) => {
      if (url !== endpoints.run && !reads.has(url)) throw new ProbeError("saved_operation_not_allowed");
      if (controller.signal.aborted) throw new ProbeError("cancelled");
      saved.checkCurrent();
      if (url === endpoints.run) {
        if (runAttempted) throw new ProbeError("saved_run_already_attempted");
        runAttempted = true;
      }
      // Request privacy, not an account mutation or a claim of server enforcement.
      return call(url, { ...options, headers: { ...options.headers, "x-ghost-mode": "true",
        "x-request-id": options.headers?.["x-request-id"] ?? randomUUID(),
        ...(compatibilityExperiment ? cliHeaders : {}) } });
    };
    return await saved.use(async (token, expected) => {
      const options = { proxyPort: args.proxyPort, signal: controller.signal };
      await verifyIdentity(token, expected, options, checkedCall);
      log("identity_verified", { accountMatched: true });
      const [catalog, config] = await settledRequests([
        models(token, options, checkedCall), inspectServerConfig(token, options, checkedCall),
      ]);
      log("models", catalog);
      log("server_config", config);
      // Reject known config blockers before optional model reads can mask them.
      requireSavedServerConfig(config, compatibilityExperiment);
      const selection = await inspectModelSelection(token, catalog, options, checkedCall);
      log("model_selection", selection);
      requireSavedModelSelection(selection);
      saved.checkCurrent();
      // inferOnce re-reads identity and the entire Free/no-alternative-billing gate.
      return inferOnce(token, expected, catalog, options, log, checkedCall);
    });
  } catch (error) {
    failure = error;
    log("error", safeError(error));
    throw error;
  } finally {
    saved?.release();
    clearTimeout(timeout);
    process.removeListener("SIGINT", stop);
    process.removeListener("SIGTERM", stop);
    try { log("probe_stopped"); }
    finally { if (!receipt.finish() && !failure) throw new ProbeError("receipt_write_failed"); }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  runSavedOnce(process.argv.slice(2)).catch((error) => {
    console.error(JSON.stringify(projectReceipt("error", safeError(error))));
    process.exitCode = 1;
  });
}
