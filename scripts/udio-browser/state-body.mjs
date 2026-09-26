// Applies to serialized runtime state, not media assets or overall process memory.
export const MAX_RUNTIME_STATE_BYTES = 16 * 1024 * 1024;

export function assertRuntimeStateSize(bytes, limit = MAX_RUNTIME_STATE_BYTES) {
  if (!Number.isSafeInteger(limit) || limit < 0) {
    throw new RangeError("Runtime-state byte limit must be a nonnegative safe integer.");
  }
  if (!Number.isSafeInteger(bytes) || bytes < 0) {
    throw new TypeError("Runtime-state byte count must be a nonnegative safe integer.");
  }
  if (bytes > limit) {
    throw Object.assign(new Error("Udio runtime state exceeds its byte limit."), {
      code: "udio_runtime_state_too_large",
      status: 413,
    });
  }
}

export async function readRuntimeStateBody(body, limit = MAX_RUNTIME_STATE_BYTES) {
  assertRuntimeStateSize(0, limit);
  if (!body) return Buffer.alloc(0);
  if (Buffer.isBuffer(body) || body instanceof Uint8Array) {
    assertRuntimeStateSize(body.byteLength, limit);
    return Buffer.isBuffer(body) ? body : Buffer.from(body);
  }
  if (typeof body[Symbol.asyncIterator] !== "function") {
    throw new TypeError("Runtime-state body must support bounded streaming.");
  }
  const chunks = [];
  let bytes = 0;
  let block = null;
  let used = 0;
  // Prefer the SDK body's iterator, never its unbounded transformToByteArray helper.
  // Exiting for-await on overflow also closes a Node Readable's iterator.
  for await (const chunk of body) {
    if (typeof chunk !== "string" && !(chunk instanceof Uint8Array)) {
      throw new TypeError("Runtime-state stream yielded a non-byte chunk.");
    }
    const length = typeof chunk === "string" ? Buffer.byteLength(chunk) : chunk.byteLength;
    assertRuntimeStateSize(bytes + length, limit);
    if (length === 0) continue;
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    // Fixed-size blocks bound metadata even when the transport yields one byte at a time.
    let offset = 0;
    while (offset < buffer.length) {
      if (!block) {
        block = Buffer.allocUnsafe(Math.min(64 * 1024, limit - bytes));
        used = 0;
      }
      const copied = buffer.copy(block, used, offset, offset + block.length - used);
      used += copied;
      offset += copied;
      bytes += copied;
      if (used === block.length) {
        chunks.push(block);
        block = null;
      }
    }
  }
  if (block) chunks.push(block.subarray(0, used));
  return Buffer.concat(chunks, bytes);
}
