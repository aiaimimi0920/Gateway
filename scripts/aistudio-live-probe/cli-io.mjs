import { mkdirSync } from "node:fs";
import path from "node:path";
import { normalizeString, stripUtf8Bom } from "./input-text.mjs";

const MAX_INPUT_BYTES = 16 * 1024 * 1024;
const INPUT_TIMEOUT_MS = 30000;

function inputError(status, code, message) {
  return Object.assign(new Error(message), { status, code });
}

function readStdin(stream = process.stdin) {
  return new Promise((resolve, reject) => {
    let buffer = null;
    let bytes = 0;
    let settled = false;
    const finish = (error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      stream.off("data", onData);
      stream.off("end", onEnd);
      stream.off("error", onError);
      stream.off("close", onClose);
      stream.pause();
      if (error) reject(error);
      else resolve(stripUtf8Bom(buffer?.toString("utf8", 0, bytes) ?? ""));
      buffer = null;
    };
    const onData = (chunk) => {
      const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, "utf8");
      if (data.length > MAX_INPUT_BYTES - bytes) {
        finish(inputError(413, "aistudio_probe_input_too_large", "AI Studio probe input exceeds 16 MiB."));
        return;
      }
      if (!data.length) return;
      // Fixed storage also bounds bookkeeping for a pipe sending tiny chunks.
      buffer ??= Buffer.allocUnsafe(MAX_INPUT_BYTES);
      data.copy(buffer, bytes);
      bytes += data.length;
    };
    const onEnd = () => finish();
    const onError = (error) => finish(error);
    const onClose = () => finish(inputError(
      400, "aistudio_probe_input_closed", "AI Studio probe input closed before EOF.",
    ));
    const timer = setTimeout(() => finish(inputError(
      408, "aistudio_probe_input_timeout", "AI Studio probe input did not finish within 30 seconds.",
    )), INPUT_TIMEOUT_MS);
    stream.on("data", onData);
    stream.once("end", onEnd);
    stream.once("error", onError);
    stream.once("close", onClose);
  });
}

function parseInput(raw) {
  try {
    return raw ? JSON.parse(raw) : {};
  } catch {
    // Native JSON diagnostics can embed credential-bearing input fragments.
    throw inputError(400, "aistudio_probe_invalid_json", "AI Studio probe input must be valid JSON.");
  }
}

function printJsonAndSetExitCode(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exitCode = exitCode;
}

function validateInput(input) {
  if (!normalizeString(input?.runtimeStateObjectKey)) {
    throw new Error("runtimeStateObjectKey is required.");
  }
}

function ensureCaptureDir(customDir) {
  const resolved =
    normalizeString(customDir) ??
    path.join(
      process.cwd(),
      ".runtime",
      "aistudio-live-probe",
      new Date().toISOString().replace(/[:.]/g, "-"),
    );
  mkdirSync(resolved, { recursive: true });
  return resolved;
}

export { readStdin, parseInput, printJsonAndSetExitCode, validateInput, ensureCaptureDir };
