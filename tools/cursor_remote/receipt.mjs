// Durable lifecycle evidence, not a credential store. Unknown fields never reach the journal.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash, randomUUID } from "node:crypto";
import { ProbeError } from "./transport.mjs";
import { safeRemoteErrorLabel } from "./connect_error.mjs";
import { safeMessageSignal, safeMessageToken } from "./error_signals.mjs";
import { safeClientVersion } from "./server_config.mjs";

const accountFields = "accountMatched accountConditionsMet membership limitType onDemandEnabled onDemandUsed includedUsed includedRemaining includedPercentUsed reasons";
const errorFields = "remoteConnectCode remoteCursorError remoteCursorDetailStatus remoteCursorRetryable remoteCursorAction remoteCursorHintsStatus remoteCursorMessageSignals remoteCursorMessageStatus remoteCursorMessageTokens remoteConnectMessageSignals remoteConnectMessageStatus remoteConnectMessageTokens";
const inferenceFields = `frames textBytes turnEnded endStream lastServerTags lastUpdateTags lastTextTags ${errorFields}`;
const fieldsByKind = {
  network: "status tlsVerified protocol",
  login_required: "expiresInSeconds",
  login_completed: "",
  saved_credential_loaded: "credentialSource",
  identity_verified: "accountMatched",
  client_compatibility_experiment: "clientProfile configVersionGate actualCliExecuted attemptLimit fallback",
  server_config: "http2Config configVersionPresent agentUrlConfigPresent agentRoute agentnRoute localLoopRoute bidiExplicitlyDisabled routeSelectionImplemented clientVersionStatusPresent clientUpdateLevel clientUpdateRequired currentClientVersionPresent minSupportedClientVersionPresent minAllowedClientVersionPresent currentClientVersion minSupportedClientVersion minAllowedClientVersion clientVersionMessagePresent",
  model_selection: "cliEntryCount cliSelectorSafe cliDefaultListed sameDefaultSelector parameterizedCount defaultNameMatches defaultAliasMatches defaultNeedsMapping defaultParameterDefinitions defaultVariants defaultVariantParameterValues defaultVariantMaxMode defaultSupportsAgent defaultRequiresDataRetention selectionMappingImplemented",
  allowance: "planInfoPresent planFree planStanding usageEnabled planUsagePresent totalPercentUsed autoUsedCents autoLimitCents autoPercentUsed apiUsedCents apiLimitCents apiPercentUsed autoBucketExhausted apiBucketExhausted autoBucketModelCount defaultInAutoBucket includedResetRaw billingCycleEndRaw entitlementVerified",
  account: accountFields, account_before_inference: accountFields, account_after: accountFields,
  models: "entryCount uniqueModelCount defaultListed selectorSafe selectorRisks",
  dashboard_diagnostics: "",
  inspection_read_failed: "operation code status",
  inspection_completed: "inferenceAttempts entitlementVerified billingBlockers modelSelectorKind selectorSafe",
  inference_start: "wireRevision requestId ghostMode webCapabilities attemptLimit fallback",
  inference_blocked: "reasons runInvocations",
  inference_failed: `code deliveryState ${errorFields} replayAllowed transportPhase transportCode`,
  inference_result: `${inferenceFields} replyMatchesExpected`,
  inference_receipt: `${inferenceFields} runInvocations`,
  account_after_unconfirmed: "code",
  session_ready: "expiresInSeconds",
  session_waiting: "runAvailable inspectionsRemaining",
  session_command_rejected: "reason",
  session_operation_failed: "operation code status",
  session_closed: "reason runRequested",
  error: "code status operation jsonReason transportPhase transportCode",
  probe_stopped: "",
};
const billingFields = "hasCreditGrants creditBalanceCents creditGrantTotalCents creditGrantUsedCents stripeFree customerBalance teamCount personalNoUsageBasedAllowed alternateLimitSource";
const blockers = "free_membership_not_confirmed personal_limit_not_confirmed on_demand_not_confirmed_off free_personal_account_not_confirmed zero_on_demand_usage_not_confirmed included_allowance_not_confirmed absence_of_credit_grants_not_confirmed absence_of_paid_balance_not_confirmed absence_of_team_billing_not_confirmed personal_usage_based_not_forbidden absence_of_alternate_limit_source_not_confirmed billing_read_failed";
const allowedText = {
  membership: "free unconfirmed", limitType: "user individual unconfirmed", protocol: "h2",
  reasons: blockers, billingBlockers: blockers,
  selectorRisks: "default_not_listed ambiguous_model_id byok_credentials unsupported_thinking_variant max_mode",
  modelSelectorKind: "unavailable account_opaque_selector named_or_unknown",
  operation: "account credit_grants hard_limit stripe teams models server_config dashboard_diagnostics inspect run cli_models cli_default_model parameter_models",
  jsonReason: "json_size_limit json_depth_limit json_duplicate_key json_nonfinite_number json_integer_form json_object_required json_syntax_or_encoding",
  credentialSource: "gateway_active_revision",
  clientProfile: "cli_20261122_e9042b2",
  configVersionGate: "diagnostic_only",
  transportPhase: "tcp_connect proxy_connect tls_handshake http2_connect request_headers response_stream",
  transportCode: "other ECONNRESET ECONNREFUSED ETIMEDOUT EPIPE ENETUNREACH EHOSTUNREACH ERR_HTTP2_ERROR ERR_HTTP2_STREAM_ERROR ERR_HTTP2_INVALID_SESSION ERR_HTTP2_GOAWAY_SESSION ERR_TLS_CERT_ALTNAME_INVALID CERT_HAS_EXPIRED UNABLE_TO_VERIFY_LEAF_SIGNATURE DEPTH_ZERO_SELF_SIGNED_CERT SELF_SIGNED_CERT_IN_CHAIN ERR_SSL_SSLV3_ALERT_HANDSHAKE_FAILURE ERR_SSL_TLSV1_ALERT_INTERNAL_ERROR",
  planStanding: "UNSPECIFIED PAST_DUE GRACE EXPIRING CANCELED CANCELED_IN_TRIAL",
  http2Config: "UNSPECIFIED FORCE_ALL_DISABLED FORCE_ALL_ENABLED FORCE_BIDI_DISABLED FORCE_BIDI_ENABLED",
  clientUpdateLevel: "UNSPECIFIED NONE RECOMMENDED REQUIRED",
  agentRoute: "absent invalid current_run_origin api_origin unapproved_origin",
  agentnRoute: "absent invalid current_run_origin api_origin unapproved_origin",
  localLoopRoute: "absent invalid current_run_origin api_origin unapproved_origin",
  deliveryState: "remote_terminal_error delivery_unknown",
  remoteCursorDetailStatus: "absent recognized invalid ambiguous unrecognized too_large",
  remoteCursorHintsStatus: "absent recognized invalid unsupported_action",
  remoteCursorMessageStatus: "absent recognized unclassified invalid too_large",
  remoteConnectMessageStatus: "absent recognized unclassified invalid too_large",
  reason: "unknown_command busy run_already_requested inspection_limit cancelled input_closed input_failed operator_quit invalid_input input_too_large session_expired",
  code: [
    "invalid_end_stream invalid_text_request unexpected_protobuf_message invalid_turn_end unsupported_connect_flags",
    "connect_frame_too_large data_after_end_stream connect_frame_limit remote_client_action_required unknown_server_event",
    "remote_tool_or_interaction_required duplicate_turn_end text_after_turn_end unexpected_text_delta invalid_text_utf8",
    "text_too_large unknown_interaction_event connect_truncated connect_remote_error successful_terminal_not_confirmed",
    "protobuf_integer_range protobuf_tag protobuf_truncated protobuf_varint protobuf_field_limit protobuf_wire_type",
    "cancelled invalid_protojson_object unknown_or_duplicate_protojson_field null_protojson_field invalid_protojson_scalar",
    "invalid_protojson_integer invalid_team_list invalid_arguments account_identity_mismatch unsupported_account_subject",
    "invalid_target target_not_allowed invalid_poll_query proxy_connection_failed proxy_truncated request_cancelled",
    "proxy_headers_too_large proxy_connect_rejected explicit_loopback_proxy_required reserved_request_header",
    "invalid_request_timeout verified_http2_required http2_request_failed unexpected_stream_content_type",
    "http2_stream_aborted http2_stream_closed response_too_large http2_reset request_timeout transport_failed",
    "upstream_http_error unexpected_json_content_type invalid_json_response invalid_model_details invalid_model_catalog",
    "account_default_model_not_verified session_token_expired invalid_session_timeout invalid_login_token",
    "invalid_token_claims invalid_or_expired_token login_poll_rejected login_expired free_inference_conditions_not_met",
    "inference_http_error inference_failed account_read_failed billing_read_failed dashboard_read_failed",
    "session_operation_failed probe_failed receipt_write_failed unknown_receipt_event receipt_already_closed",
    "invalid_saved_selection saved_path_redirected saved_path_invalid saved_file_invalid saved_file_changed",
    "saved_credential_read_failed saved_revision_changed saved_archive_invalid saved_credential_not_import_only",
    "saved_expiry_mismatch saved_credential_released saved_operation_not_readonly saved_inspection_incomplete invalid_server_config invalid_selection_metadata",
    "saved_operation_not_allowed saved_run_already_attempted saved_run_route_unconfirmed saved_run_selection_unconfirmed",
    "invalid_allowance_summary saved_run_client_update_required invalid_client_compatibility_experiment unexpected_protobuf_content_type",
    "unsupported_http_content_encoding invalid_compressed_response",
  ].join(" "),
};
const textSets = Object.fromEntries(Object.entries(allowedText).map(([key, values]) => [key, new Set(values.split(" "))]));
function scalar(key, value) {
  if (["currentClientVersion", "minSupportedClientVersion", "minAllowedClientVersion"].includes(key)) {
    return value === undefined ? undefined : value === null ? null : safeClientVersion(value);
  }
  if (key === "requestId") return typeof value === "string" &&
    /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(value) ? value : undefined;
  if (["remoteCursorMessageSignals", "remoteConnectMessageSignals"].includes(key)) {
    return typeof value === "string" ? safeMessageSignal(value) : undefined;
  }
  if (["remoteCursorMessageTokens", "remoteConnectMessageTokens"].includes(key)) {
    return typeof value === "string" ? safeMessageToken(value) : undefined;
  }
  if (key === "remoteCursorRetryable") return typeof value === "boolean" ? value : null;
  if (["remoteConnectCode", "remoteCursorError", "remoteCursorAction"].includes(key)) {
    return safeRemoteErrorLabel(key, value);
  }
  if (value === null || typeof value === "boolean" || (typeof value === "number" && Number.isFinite(value))) return value;
  if (typeof value !== "string") return undefined;
  return textSets[key]?.has(value) ? value : "unrecognized";
}
function select(data, fields) {
  const result = {};
  for (const key of fields.split(" ")) {
    const value = data?.[key];
    const projected = scalar(key, value);
    if (projected !== undefined) result[key] = projected;
    else if (Array.isArray(value) && value.length <=
      (["remoteCursorMessageTokens", "remoteConnectMessageTokens"].includes(key) ? 96 : 32)) {
      result[key] = value.map((item) => scalar(key, item) ?? "unrecognized");
    }
  }
  return result;
}

export function projectReceipt(kind, data = {}) {
  if (!Object.hasOwn(fieldsByKind, kind)) throw new ProbeError("unknown_receipt_event");
  const result = { kind, ...select(data, fieldsByKind[kind]) };
  if (["account", "account_before_inference", "account_after"].includes(kind) && data.billing) {
    result.billing = select(data.billing, billingFields);
    result.billing.readErrors = Array.isArray(data.billing.errors) ? data.billing.errors.length : null;
  }
  return result;
}

function observe(state, event) {
  const { kind } = event;
  if (kind === "login_required") state.phase = "authorization";
  if (kind === "login_completed" || kind === "saved_credential_loaded") {
    state.authReceived = true;
    state.credentialSource = kind === "login_completed" ? "new_authorization" : event.credentialSource;
    state.phase = "account";
  }
  if (kind === "identity_verified" && event.accountMatched === true) state.accountMatched = true;
  if (["account", "account_before_inference", "account_after"].includes(kind)) {
    if (event.accountMatched === true) state.accountMatched = true;
    if (kind === "account") state.phase = "catalog";
    if (kind === "account_before_inference") state.phase = "preflight";
  }
  if (kind === "models") state.phase = "preflight";
  if (kind === "inference_start") {
    state.phase = "inference";
    state.inferenceAttempts++;
    state.inferenceStatus = "started";
  }
  if (kind === "inference_result") state.inferenceStatus = "succeeded";
  if (kind === "inference_blocked") state.inferenceStatus = "blocked";
  if (kind === "inference_failed") {
    state.inferenceStatus = "failed";
    Object.assign(state, select(event, fieldsByKind.inference_failed));
  }
  if (kind === "error" || kind === "session_operation_failed") {
    state.failureStage = state.phase;
    state.errorCode = event.code;
  }
  if (kind === "session_closed") state.stopReason = event.reason;
  if (kind === "probe_stopped") {
    state.credentialRetention = "not_retained_by_probe";
    state.stopReason ??= state.errorCode ?? "completed";
  }
}

function sourceHashes() {
  const directory = fileURLToPath(new URL("./", import.meta.url));
  return Object.fromEntries(fs.readdirSync(directory).filter((name) => name.endsWith(".mjs")).sort()
    .map((name) => [`tools/cursor_remote/${name}`,
      createHash("sha256").update(fs.readFileSync(path.join(directory, name))).digest("hex")]));
}

function evidenceDirectory() {
  const root = fs.realpathSync(fileURLToPath(new URL("../../", import.meta.url)));
  if (process.platform === "win32" && !/^C:\\/i.test(root)) throw new ProbeError("receipt_requires_local_c");
  let directory = root;
  for (const name of ["target", "provider-evidence"]) {
    directory = path.join(directory, name);
    try { fs.mkdirSync(directory); } catch (error) { if (error.code !== "EEXIST") throw error; }
    // Reject reparse/symlink traversal rather than writing evidence outside this checkout.
    if (fs.lstatSync(directory).isSymbolicLink() || fs.realpathSync(directory) !== directory) {
      throw new ProbeError("receipt_directory_redirected");
    }
  }
  return directory;
}

export function createProbeReporter({ mode, controller, directory, emit = console.log, io = fs,
  emitProjection = false }) {
  const attemptId = randomUUID();
  let file, fd;
  try {
    file = path.join(directory ?? evidenceDirectory(), `cursor-remote-${attemptId}.jsonl`);
    fd = io.openSync(file, "wx", 0o600);
  } catch { throw new ProbeError("receipt_open_failed"); }
  let failed = false, closed = false, rows = 0, bytes = 0;
  const state = { phase: "starting", authReceived: false, accountMatched: false,
    credentialRetention: "memory_only", inferenceAttempts: 0, inferenceStatus: "not_started",
    inferenceAttemptsMeaning: "local_transport_attempts_not_delivery",
    credentialsPersisted: false, replayAllowed: false, actualModel: null, freeAttribution: "not_proven" };
  const write = (event) => {
    if (failed) return;
    try {
      const buffer = Buffer.from(JSON.stringify({ attemptId, observedAt: new Date().toISOString(), ...event }) + "\n");
      if (++rows > 256 || (bytes += buffer.length) > 1024 * 1024) throw new Error("receipt_limit");
      let offset = 0;
      while (offset < buffer.length) {
        const written = io.writeSync(fd, buffer, offset, buffer.length - offset);
        if (written <= 0) throw new Error("receipt_short_write");
        offset += written;
      }
      io.fsyncSync(fd);
    } catch {
      // Do not replace an existing upstream error or skip cleanup. Cancellation prevents new work.
      failed = true;
      controller.abort();
    }
  };
  try {
    write({ kind: "probe_started", schemaVersion: 1, mode, pid: process.pid, sourceSha256: sourceHashes() });
    if (failed) throw new ProbeError("receipt_write_failed");
    emit(JSON.stringify({ kind: "receipt_opened", attemptId, path: file }));
  } catch (error) {
    try { io.closeSync(fd); } catch { /* Startup already failed; no authorization was initiated. */ }
    throw error;
  }
  return {
    file,
    log(kind, data = {}) {
      if (closed) throw new ProbeError("receipt_already_closed");
      const event = projectReceipt(kind, data);
      write(event);
      // A failed admission write cancels before inferOnce can call transport.
      if (kind !== "inference_start" || !controller.signal.aborted) observe(state, event);
      // Only the interactive channel gets the public challenge; never persist it.
      emit(JSON.stringify({ attemptId, kind,
        ...(emitProjection || (kind === "login_required" && controller.signal.aborted) ? event : data) }));
    },
    finish() {
      if (closed) throw new ProbeError("receipt_already_closed");
      closed = true;
      write({ kind: "probe_summary", ...state, receiptComplete: true });
      const receiptComplete = !failed;
      let receiptClosed = true;
      try { io.closeSync(fd); } catch { receiptClosed = false; controller.abort(); }
      emit(JSON.stringify({ kind: "probe_summary", attemptId, ...state, receiptComplete, receiptClosed,
        ...(!receiptComplete || !receiptClosed ? { receiptError: "receipt_write_failed" } : {}) }));
      return receiptComplete && receiptClosed;
    },
  };
}
