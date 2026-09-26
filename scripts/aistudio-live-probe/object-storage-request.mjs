export const MAX_RUNTIME_STATE_BYTES = 16 * 1024 * 1024;

function storageError(code, message, status) {
  return Object.assign(new Error(message), { code, status });
}

export function runtimeStateTooLarge() {
  return storageError("aistudio_runtime_state_too_large", "AI Studio runtime state exceeds 16 MiB.", 413);
}

export async function withObjectStorageRequest(operation) {
  const controller = new AbortController();
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => {
      const error = storageError("aistudio_object_storage_timeout", "AI Studio object storage exceeded 30 seconds.", 504);
      controller.abort(error);
      reject(error);
    }, 30000);
  });
  try {
    return await Promise.race([Promise.resolve().then(() => operation(controller.signal)), deadline]);
  } catch (error) {
    if (controller.signal.aborted) throw controller.signal.reason;
    if (error?.code === "aistudio_runtime_state_too_large") throw runtimeStateTooLarge();
    throw storageError("aistudio_object_storage_failed", "AI Studio object storage request failed.", 502);
  } finally {
    clearTimeout(timer);
  }
}

export async function readRuntimeStateBody(body, signal, declaredLength) {
  const destroy = () => { if (typeof body?.destroy === "function") body.destroy(); };
  if (signal.aborted) { destroy(); throw signal.reason; }
  if (Number(declaredLength) > MAX_RUNTIME_STATE_BYTES) { destroy(); throw runtimeStateTooLarge(); }
  if (!body) return Buffer.alloc(0);
  if (Buffer.isBuffer(body)) {
    if (body.length > MAX_RUNTIME_STATE_BYTES) throw runtimeStateTooLarge();
    return body;
  }
  if (typeof body[Symbol.asyncIterator] !== "function" || typeof body.destroy !== "function") {
    throw new Error("Object storage must return a cancellable Node readable body.");
  }
  let buffer = null;
  let bytes = 0;
  signal.addEventListener("abort", destroy, { once: true });
  try {
    for await (const chunk of body) {
      signal.throwIfAborted();
      const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      if (data.length > MAX_RUNTIME_STATE_BYTES - bytes) throw runtimeStateTooLarge();
      if (!data.length) continue;
      if (bytes + data.length > (buffer?.length ?? 0)) {
        const capacity = Math.min(MAX_RUNTIME_STATE_BYTES,
          Math.max(65536, bytes + data.length, (buffer?.length ?? 0) * 2));
        const next = Buffer.allocUnsafe(capacity);
        buffer?.copy(next, 0, 0, bytes);
        buffer = next;
      }
      data.copy(buffer, bytes);
      bytes += data.length;
    }
    signal.throwIfAborted();
    return buffer?.subarray(0, bytes) ?? Buffer.alloc(0);
  } catch (error) {
    destroy();
    if (signal.aborted) throw signal.reason;
    throw error;
  } finally {
    signal.removeEventListener("abort", destroy);
  }
}
