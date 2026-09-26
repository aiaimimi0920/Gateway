const MAX_RESPONSE_BYTES = 16 * 1024 * 1024;

export async function readNodeResponseText(response, signal) {
  if (signal.aborted) throw signal.reason;
  if (!response.body) return "";
  const reader = response.body.getReader();
  let buffer = null;
  let bytes = 0;
  // Explicit cancellation unblocks a pending body read after headers arrived.
  const cancel = () => { void reader.cancel(signal.reason).catch(() => undefined); };
  signal.addEventListener("abort", cancel, { once: true });
  try {
    while (true) {
      if (signal.aborted) throw signal.reason;
      const { value, done } = await reader.read();
      if (signal.aborted) throw signal.reason;
      if (done) break;
      if (value.byteLength > MAX_RESPONSE_BYTES - bytes) {
        throw Object.assign(new Error("Producer response exceeds 16 MiB."), {
          code: "producer_node_response_too_large",
        });
      }
      if (!value.byteLength) continue;
      const required = bytes + value.byteLength;
      if (!buffer || required > buffer.byteLength) {
        const capacity = Math.min(
          MAX_RESPONSE_BYTES,
          Math.max(64 * 1024, required, (buffer?.byteLength ?? 0) * 2),
        );
        const grown = Buffer.allocUnsafe(capacity);
        if (buffer) grown.set(buffer.subarray(0, bytes));
        buffer = grown;
      }
      buffer.set(value, bytes);
      bytes += value.byteLength;
    }
    return buffer?.toString("utf8", 0, bytes) ?? "";
  } finally {
    signal.removeEventListener("abort", cancel);
    await reader.cancel().catch(() => undefined);
    reader.releaseLock();
  }
}
