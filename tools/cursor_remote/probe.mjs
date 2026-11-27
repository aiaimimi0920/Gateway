// Explicit, ephemeral experiment. No runtime credentials are read; only safe receipts are printed.
import { pathToFileURL } from "node:url";
import { challenge, login, tokenClaims } from "./auth.mjs";
import { account, models } from "./qualification.mjs";
import { inferenceReasons } from "./billing.mjs";
import { inspectDashboard } from "./diagnostics.mjs";
import { inferOnce, runWithFreshCatalog } from "./inference.mjs";
import { inspectSession, sessionDuration, watchLoginInput } from "./session.mjs";
import { createProbeReporter } from "./receipt.mjs";
import { endpoints, ProbeError, request } from "./transport.mjs";

export function argumentsFor(argv) {
  const parsed = {};
  for (let i = 0; i < argv.length; i += 2) {
    const key = argv[i], value = argv[i + 1];
    if (!["--proxy-port", "--expected-email-sha256", "--mode"].includes(key) ||
        value === undefined || Object.hasOwn(parsed, key)) throw new ProbeError("invalid_arguments");
    parsed[key] = value;
  }
  const port = Number(parsed["--proxy-port"]);
  const mode = parsed["--mode"];
  if (!Number.isInteger(port) || port < 1 || port > 65535 ||
      !["network", "account", "once", "session"].includes(mode) ||
      (mode !== "network" && !/^[a-f0-9]{64}$/.test(parsed["--expected-email-sha256"] ?? ""))) {
    throw new ProbeError("invalid_arguments");
  }
  return { proxyPort: port, mode, expected: parsed["--expected-email-sha256"] };
}

export async function inspectRemote(token, expected, options, log, call = request) {
  const snapshot = await account(token, expected, options, call);
  log("account", snapshot);
  const results = await Promise.allSettled([
    models(token, options, call), inspectDashboard(token, options, call),
  ]);
  for (const [index, result] of results.entries()) {
    const operation = ["models", "dashboard_diagnostics"][index];
    if (result.status === "fulfilled") log(operation, result.value);
    else log("inspection_read_failed", { operation, code: safeCode(result.reason),
      ...(result.reason instanceof ProbeError && result.reason.status ? { status: result.reason.status } : {}) });
  }
  const catalog = results[0].status === "fulfilled" ? results[0].value : null;
  log("inspection_completed", { inferenceAttempts: 0, entitlementVerified: false,
    billingBlockers: inferenceReasons(snapshot),
    modelSelectorKind: !catalog ? "unavailable" : /^(auto|default)$/i.test(catalog.defaultModel ?? "")
      ? "account_opaque_selector" : "named_or_unknown",
    selectorSafe: catalog?.selectorSafe === true });
}

export async function main(argv, { call = request, input = process.stdin, reporter = createProbeReporter } = {}) {
  const args = argumentsFor(argv);
  const controller = new AbortController();
  // Open durable evidence before creating a challenge. The CLI exposes no output-path override.
  const receipt = reporter({ mode: args.mode, controller });
  const stop = () => controller.abort();
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  const options = { proxyPort: args.proxyPort, signal: controller.signal };
  const log = receipt.log;
  let token;
  let failure;
  let releaseLoginInput = () => {};
  try {
    if (args.mode === "network") {
      const response = await call(endpoints.network, { ...options, method: "HEAD" });
      log("network", { status: response.status, tlsVerified: true, protocol: "h2" });
      return;
    }
    if (args.mode === "session") releaseLoginInput = watchLoginInput(input, controller);
    if (controller.signal.aborted) throw new ProbeError("cancelled");
    const auth = challenge();
    // Public challenge only: never log verifier, token, refresh token, cookie or upstream bodies.
    log("login_required", { url: auth.url, expiresInSeconds: 600, credentialsPersisted: false });
    try {
      if (controller.signal.aborted) throw new ProbeError("cancelled");
      token = await login(auth, options, call);
    }
    finally { auth.verifier = ""; auth.url = ""; }
    log("login_completed", { credentialsPersisted: false });
    if (controller.signal.aborted) throw new ProbeError("cancelled");
    if (args.mode === "session") {
      const session = inspectSession({ input, controller,
        ttlMs: sessionDuration(tokenClaims(token).exp),
        inspect: () => inspectRemote(token, args.expected, options, log, call),
        run: () => runWithFreshCatalog(token, args.expected, options, log, call),
        log,
      });
      // Session listeners are installed before removing the login guard: no cancellation gap.
      releaseLoginInput();
      await session;
      return;
    }
    const before = await account(token, args.expected, options, call);
    log("account", before);
    const catalog = await models(token, options, call);
    log("models", catalog);
    log("inspection_completed", { inferenceAttempts: 0, entitlementVerified: false });
    if (args.mode === "once") {
      // A fresh account/billing read decides admission, never directory visibility alone.
      await inferOnce(token, args.expected, catalog, options, log, call);
    }
  } catch (error) {
    failure = error;
    log("error", { code: safeCode(error),
      ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) });
    throw error;
  } finally {
    releaseLoginInput();
    if (args.mode === "session") input.pause();
    token = undefined;
    process.removeListener("SIGINT", stop);
    process.removeListener("SIGTERM", stop);
    try { log("probe_stopped", { credentialsPersisted: false }); }
    finally {
      if (!receipt.finish() && !failure) throw new ProbeError("receipt_write_failed");
    }
  }
}

export function safeCode(error) {
  return error instanceof ProbeError ? error.code : "probe_failed";
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main(process.argv.slice(2)).catch((error) => {
    console.error(JSON.stringify({ kind: "error", code: safeCode(error),
      ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) }));
    process.exitCode = 1;
  });
}
