const MAX_INPUT_BYTES = 16 * 1024 * 1024;
const INPUT_TIMEOUT_MS = 30000;

function inputError(status, code, message) {
  return Object.assign(new Error(message), { status, code });
}

export function readStdin(stream = process.stdin) {
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
      else resolve(buffer?.toString("utf8", 0, bytes) ?? "");
      buffer = null;
    };
    const onData = (chunk) => {
      const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, "utf8");
      if (data.length > MAX_INPUT_BYTES - bytes) {
        finish(inputError(413, "producer_browser_input_too_large", "Producer worker input exceeds 16 MiB."));
        return;
      }
      if (!data.length) return;
      // A fixed buffer bounds bookkeeping even when a pipe sends tiny chunks.
      buffer ??= Buffer.allocUnsafe(MAX_INPUT_BYTES);
      data.copy(buffer, bytes);
      bytes += data.length;
    };
    const onEnd = () => finish();
    const onError = (error) => finish(error);
    const onClose = () => finish(inputError(400, "producer_browser_input_closed", "Producer worker input closed before EOF."));
    const timer = setTimeout(() => finish(inputError(
      408, "producer_browser_input_timeout", "Producer worker input did not finish within 30 seconds.",
    )), INPUT_TIMEOUT_MS);
    stream.on("data", onData);
    stream.once("end", onEnd);
    stream.once("error", onError);
    stream.once("close", onClose);
  });
}
