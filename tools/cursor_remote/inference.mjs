// One authorized experimental ASK. No retry, refresh, tool execution or model/account fallback.
import { randomUUID } from "node:crypto";
import { account, models, verifyIdentity } from "./qualification.mjs";
import { requireAccountModel } from "./catalog_wire.mjs";
import { inferenceReasons } from "./billing.mjs";
import { expectedReply, TextDecoderState, textRequest } from "./protocol.mjs";
import { endpoints, ProbeError, request } from "./transport.mjs";

export async function runWithFreshCatalog(token, expected, options, log, call = request) {
  // Even catalog reads require identity binding; inferOnce rechecks billing before any Run.
  await verifyIdentity(token, expected, options, call);
  return inferOnce(token, expected, await models(token, options, call), options, log, call);
}

export async function inferOnce(token, expected, catalog, options, log, call = request) {
  const model = requireAccountModel(catalog);
  const immediate = await account(token, expected, options, call);
  log("account_before_inference", immediate);
  const reasons = inferenceReasons(immediate);
  if (reasons.length) {
    log("inference_blocked", { reasons, runInvocations: 0 });
    throw new ProbeError("free_inference_conditions_not_met");
  }
  const decoder = new TextDecoderState({ allowSessionMessages: true });
  const requestId = randomUUID(), body = textRequest(model);
  if (options.signal?.aborted) throw new ProbeError("cancelled");
  log("inference_start", { requestedModel: model, wireRevision: 4, requestId,
    ghostMode: true, webCapabilities: false, attemptLimit: 1, fallback: false });
  if (options.signal?.aborted) throw new ProbeError("cancelled");
  try {
    const response = await call(endpoints.run, {
      ...options, timeoutMs: 60000, duplex: true,
      headers: { authorization: `Bearer ${token}`, "content-type": "application/connect+proto",
        accept: "application/connect+proto", "connect-protocol-version": "1",
        "connect-accept-encoding": "identity", "x-ghost-mode": "true", "x-request-id": requestId },
      body, onChunk: (chunk) => { decoder.push(chunk); return decoder.takeReplies(); },
    });
    if (response.status !== 200) throw new ProbeError("inference_http_error", response.status);
    const result = { ...decoder.finish(), requestedModel: model, actualModel: null,
      freeAttribution: "not_proven_by_protocol" };
    // Generated output is untrusted too: retain it only in memory, not in receipts.
    log("inference_result", { ...decoder.receipt(), requestedModel: model, actualModel: null,
      freeAttribution: result.freeAttribution, replyMatchesExpected: result.text.trim() === expectedReply });
    return result;
  } catch (error) {
    // A terminal error does not prove zero execution, no charge or safe replay.
    log("inference_failed", { code: error instanceof ProbeError ? error.code : "inference_failed",
      transportPhase: error instanceof ProbeError ? error.transportPhase : undefined,
      transportCode: error instanceof ProbeError ? error.transportCode : undefined,
      deliveryState: error instanceof ProbeError && error.code === "connect_remote_error"
        ? "remote_terminal_error" : "delivery_unknown",
      remoteConnectCode: decoder.remoteConnectCode, remoteCursorError: decoder.remoteCursorError,
      remoteCursorDetailStatus: decoder.remoteCursorDetailStatus,
      remoteCursorRetryable: decoder.remoteCursorRetryable, remoteCursorAction: decoder.remoteCursorAction,
      remoteCursorMessageSignals: decoder.remoteCursorMessageSignals,
      remoteCursorMessageStatus: decoder.remoteCursorMessageStatus,
      ...decoder.messageProjection,
      remoteCursorHintsStatus: decoder.remoteCursorHintsStatus, replayAllowed: false });
    throw error;
  } finally {
    decoder.close();
    log("inference_receipt", { ...decoder.receipt(), runInvocations: 1 });
    if (!options.signal?.aborted) {
      try { log("account_after", await account(token, expected, options, call)); }
      catch (error) {
        log("account_after_unconfirmed",
          { code: error instanceof ProbeError ? error.code : "account_read_failed" });
      }
    }
  }
}
