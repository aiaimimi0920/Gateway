// A bounded operator pipe, not a server: fixed commands, no eval, hot reload or token export.
import { ProbeError } from "./transport.mjs";

export function sessionDuration(exp, now = Date.now()) {
  const ttl = Math.min(5 * 60000, Math.floor(exp * 1000 - now - 60000));
  if (!Number.isFinite(ttl) || ttl <= 0) throw new ProbeError("session_token_expired");
  return ttl;
}

export function watchLoginInput(input, controller) {
  const stop = () => controller.abort();
  // Drain pre-login input, but never queue it as a command for the authenticated session.
  const discard = () => {};
  input.on("data", discard);
  input.once("end", stop);
  input.once("close", stop);
  input.once("error", stop);
  if (input.readableEnded || input.destroyed) stop();
  return () => {
    input.off("data", discard);
    input.off("end", stop);
    input.off("close", stop);
    input.off("error", stop);
  };
}

export async function inspectSession({ input, controller, ttlMs, inspect, run, log }) {
  if (!Number.isInteger(ttlMs) || ttlMs < 1 || ttlMs > 5 * 60000) {
    throw new ProbeError("invalid_session_timeout");
  }
  let buffer = "", busy = false, runConsumed = false, inspections = 0, pending;
  let stopReason;
  let complete;
  const stopped = new Promise((resolve) => { complete = resolve; });
  const stop = (reason) => {
    if (stopReason) return;
    stopReason = reason;
    buffer = "";
    controller.abort();
    complete();
  };
  const aborted = () => stop("cancelled");
  const ended = () => stop("input_closed");
  const errored = () => stop("input_failed");
  const execute = (command) => {
    if (stopReason) return;
    if (command === "quit") return stop("operator_quit");
    if (command !== "inspect" && command !== "run") {
      log("session_command_rejected", { reason: "unknown_command" });
      return;
    }
    if (busy) {
      log("session_command_rejected", { reason: "busy" });
      return;
    }
    if (command === "run" && runConsumed) {
      log("session_command_rejected", { reason: "run_already_requested" });
      return;
    }
    if (command === "inspect" && inspections >= 3) {
      log("session_command_rejected", { reason: "inspection_limit" });
      return;
    }
    // Consume before awaiting any preflight: even a blocked run cannot be requested twice.
    if (command === "run") runConsumed = true;
    else inspections++;
    busy = true;
    pending = Promise.resolve().then(() => {
      if (!controller.signal.aborted) return command === "run" ? run() : inspect();
    })
      .catch((error) => {
        log("session_operation_failed", { operation: command,
          code: error instanceof ProbeError ? error.code : "session_operation_failed",
          ...(error instanceof ProbeError && error.status ? { status: error.status } : {}) });
      }).finally(() => {
        busy = false;
        if (!stopReason) log("session_waiting", { runAvailable: !runConsumed,
          inspectionsRemaining: 3 - inspections });
      });
  };
  const data = (chunk) => {
    // Reject before buffering: no unbounded readline queue or command backlog.
    if (!Buffer.isBuffer(chunk)) return stop("invalid_input");
    if (chunk.length > 128) return stop("input_too_large");
    for (const byte of chunk) {
      if (stopReason) return;
      if (byte === 10) {
        const command = buffer.endsWith("\r") ? buffer.slice(0, -1) : buffer;
        buffer = "";
        execute(command);
      } else if ((byte < 32 && byte !== 13) || byte > 126 || buffer.length >= 16) {
        stop("invalid_input");
      } else buffer += String.fromCharCode(byte);
    }
  };
  const timer = setTimeout(() => stop("session_expired"), ttlMs);
  controller.signal.addEventListener("abort", aborted, { once: true });
  input.on("data", data);
  input.once("end", ended);
  input.once("close", ended);
  input.once("error", errored);
  try {
    if (controller.signal.aborted) stop("cancelled");
    else if (input.readableEnded || input.destroyed) stop("input_closed");
    else {
      log("session_ready", { expiresInSeconds: Math.ceil(ttlMs / 1000),
        commands: ["inspect", "run", "quit"], credentialsPersisted: false });
      execute("inspect");
    }
    await stopped;
    // Cancelling ends admission immediately, then awaits transport cleanup before dropping token.
    await pending;
  } finally {
    clearTimeout(timer);
    controller.signal.removeEventListener("abort", aborted);
    input.off("data", data);
    input.off("end", ended);
    input.off("close", ended);
    input.off("error", errored);
    input.pause();
    log("session_closed", { reason: stopReason, runRequested: runConsumed,
      credentialsPersisted: false });
  }
}
