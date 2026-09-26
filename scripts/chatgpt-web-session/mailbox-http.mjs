export async function fetchMailboxResponse(url, options, overflowCode = "mailbox_response_too_large") {
  const controller = new AbortController();
  const parentSignal = options?.signal;
  const onParentAbort = () => controller.abort(parentSignal.reason);
  const timer = setTimeout(() => controller.abort(), 15_000);
  try {
    parentSignal?.throwIfAborted();
    parentSignal?.addEventListener("abort", onParentAbort, { once: true });
    const response = await fetch(url, { ...options, signal: controller.signal, redirect: "error" });
    const text = await readMailboxResponse(response, controller.signal, overflowCode);
    return { status: response.status, ok: response.ok, text };
  } finally {
    parentSignal?.removeEventListener("abort", onParentAbort);
    clearTimeout(timer);
    controller.abort();
  }
}

export async function fetchMailboxJson(url, options, statusPrefix) {
  const response = await fetchMailboxResponse(url, options);
  if (!response.ok) throw new Error(`${statusPrefix}_${response.status}`);
  try {
    return JSON.parse(response.text);
  } catch {
    return {};
  }
}

async function readMailboxResponse(response, signal, overflowCode) {
  if (!response.body) return "";
  const reader = response.body.getReader();
  const buffer = Buffer.allocUnsafe(4 * 1024 * 1024);
  let bytes = 0;
  // Cancel the acquired reader too: aborting fetch after headers can leave
  // body reads pending on some runtimes.
  const onAbort = () => { void reader.cancel().catch(() => {}); };
  signal.addEventListener("abort", onAbort, { once: true });
  try {
    while (true) {
      signal.throwIfAborted();
      const { done, value } = await reader.read();
      signal.throwIfAborted();
      if (done) return buffer.toString("utf8", 0, bytes);
      if (bytes + value.byteLength > buffer.length) throw new Error(overflowCode);
      buffer.set(value, bytes);
      bytes += value.byteLength;
    }
  } finally {
    signal.removeEventListener("abort", onAbort);
    await reader.cancel().catch(() => {});
    reader.releaseLock();
  }
}
