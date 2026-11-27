// Explicit read-only reuse of one already imported account. No login, refresh, Run or mutation path.
import { pathToFileURL } from "node:url";
import { openSavedCredential } from "./saved_credential.mjs";
import { account, models, verifyIdentity } from "./qualification.mjs";
import { inferenceReasons } from "./billing.mjs";
import { inspectServerConfig } from "./server_config.mjs";
import { inspectModelSelection } from "./model_selection.mjs";
import { inspectAllowance } from "./allowance.mjs";
import { createProbeReporter, projectReceipt } from "./receipt.mjs";
import { endpoints, ProbeError, request, settledRequests } from "./transport.mjs";

const keys = ["--data-root", "--revision", "--provider", "--credential", "--proxy-port"];
const reads = new Set(["me", "usage", "grants", "stripe", "teams", "hardLimit", "models",
  "defaultModel", "serverConfig", "cliModels", "cliDefaultModel", "parameterModels", "plan",
  "currentPeriod"].map((key) => endpoints[key]));

export function savedArguments(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i += 2) {
    if (!keys.includes(argv[i]) || argv[i + 1] === undefined || Object.hasOwn(args, argv[i])) {
      throw new ProbeError("invalid_arguments");
    }
    args[argv[i]] = argv[i + 1];
  }
  const proxyPort = Number(args["--proxy-port"]);
  if (keys.some((key) => !args[key]) || !Number.isInteger(proxyPort) || proxyPort < 1 || proxyPort > 65535) {
    throw new ProbeError("invalid_arguments");
  }
  return { dataRoot: args["--data-root"], revision: args["--revision"],
    providerId: args["--provider"], credentialId: args["--credential"], proxyPort };
}

const safeError = (error) => ({ code: error instanceof ProbeError ? error.code : "probe_failed",
  ...(error instanceof ProbeError ? { operation: error.operation, jsonReason: error.jsonReason } : {}),
  ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) });

export async function inspectSaved(argv, { call = request, open = openSavedCredential,
  reporter = createProbeReporter } = {}) {
  const args = savedArguments(argv), controller = new AbortController();
  const receipt = reporter({ mode: "saved-account", controller, emitProjection: true });
  // Unlike interactive PKCE output, every field on this path passes the durable projection.
  const log = (kind, data) => receipt.log(kind, data);
  const stop = () => controller.abort();
  const timeout = setTimeout(stop, 60000);
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  let saved, failure;
  try {
    saved = open(args);
    log("saved_credential_loaded", { credentialSource: "gateway_active_revision" });
    const checkedCall = async (url, options) => {
      if (!reads.has(url)) throw new ProbeError("saved_operation_not_readonly");
      if (controller.signal.aborted) throw new ProbeError("cancelled");
      saved.checkCurrent();
      return call(url, options);
    };
    await saved.use(async (token, expected) => {
      const options = { proxyPort: args.proxyPort, signal: controller.signal };
      await verifyIdentity(token, expected, options, checkedCall);
      log("identity_verified", { accountMatched: true });
      const operations = [
        ["account", () => account(token, expected, options, checkedCall)],
        ["models", () => models(token, options, checkedCall)],
        ["server_config", () => inspectServerConfig(token, options, checkedCall)],
      ];
      // Always settle siblings before releasing the in-memory credential.
      const results = await Promise.allSettled(operations.map(async ([kind, work]) => {
        const value = await work();
        log(kind, value);
        return value;
      }));
      for (const [index, result] of results.entries()) {
        if (result.status === "rejected") log("inspection_read_failed",
          { operation: operations[index][0], ...safeError(result.reason) });
      }
      const snapshot = results[0].status === "fulfilled" ? results[0].value : null;
      const catalog = results[1].status === "fulfilled" ? results[1].value : null;
      for (const error of snapshot?.billing.errors ?? []) log("inspection_read_failed", error);
      if (catalog && !controller.signal.aborted) {
        await settledRequests([
          inspectModelSelection(token, catalog, options, checkedCall).then((value) => log("model_selection", value)),
          inspectAllowance(token, catalog.defaultModel, options, checkedCall).then((value) => log("allowance", value)),
        ]);
      }
      saved.checkCurrent();
      if (controller.signal.aborted) throw new ProbeError("cancelled");
      log("inspection_completed", { inferenceAttempts: 0, entitlementVerified: false,
        billingBlockers: snapshot ? inferenceReasons(snapshot) : ["billing_read_failed"],
        selectorSafe: catalog?.selectorSafe === true,
        modelSelectorKind: !catalog ? "unavailable" : /^(auto|default)$/i.test(catalog.defaultModel ?? "")
          ? "account_opaque_selector" : "named_or_unknown" });
      if (results.some((result) => result.status === "rejected") || snapshot?.billing.errors.length) {
        throw new ProbeError("saved_inspection_incomplete");
      }
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
  inspectSaved(process.argv.slice(2)).catch((error) => {
    console.error(JSON.stringify(projectReceipt("error", safeError(error))));
    process.exitCode = 1;
  });
}
