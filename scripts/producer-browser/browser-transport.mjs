// Playwright serializes this callback independently of the Node module scope.
export async function executeBrowserTransport({ url, options, timeoutMs, stopOnTerminal }) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
  let reader;
  const cancel = () => { void reader?.cancel(controller.signal.reason).catch(() => undefined); };
  try {
    const response = await fetch(url, {
      ...options,
      redirect: "error",
      signal: controller.signal,
    });
    const limit = 16 * 1024 * 1024;
    let buffer = null;
    let bytes = 0;
    if (response.body) {
      reader = response.body.getReader();
      controller.signal.addEventListener("abort", cancel, { once: true });
      const streamDecoder = new TextDecoder();
      let prefix = 0;
      let lineNonWhitespace = false;
      let eventValue = "";
      let valueEnded = false;
      let valueInvalid = false;
      let frameEvent = "";
      let previousCR = false;
      // Inspect each decoded character once, retaining only event-name state.
      // Cancellation waits for the blank line so fragmented terminal data survives.
      const consumeTerminalFrames = (text) => {
        for (const character of text) {
          if (previousCR && character === "\n") {
            previousCR = false;
            continue;
          }
          previousCR = character === "\r";
          if (character === "\r" || character === "\n") {
            const terminal = !lineNonWhitespace && (frameEvent === "final" || frameEvent === "error");
            if (!lineNonWhitespace) frameEvent = "";
            else if (prefix === 6) frameEvent = valueInvalid ? "" : eventValue;
            prefix = 0;
            lineNonWhitespace = false;
            eventValue = "";
            valueEnded = false;
            valueInvalid = false;
            if (terminal) return true;
            continue;
          }
          if (!lineNonWhitespace && character.trim()) lineNonWhitespace = true;
          if (prefix < 0) continue;
          if (prefix < 6) {
            prefix = character === "event:"[prefix] ? prefix + 1 : -1;
            continue;
          }
          if (valueInvalid) continue;
          if (!character.trim()) {
            if (eventValue) valueEnded = true;
          } else if (valueEnded || eventValue.length >= 5) {
            valueInvalid = true;
          } else {
            eventValue += character;
          }
        }
        return false;
      };
      while (true) {
        if (controller.signal.aborted) throw controller.signal.reason;
        const { value, done } = await reader.read();
        if (controller.signal.aborted) throw controller.signal.reason;
        if (done) break;
        if (value.byteLength > limit - bytes) {
          throw Object.assign(new Error("Producer browser response exceeds 16 MiB."), {
            code: "producer_browser_response_too_large",
          });
        }
        if (!value.byteLength) continue;
        const required = bytes + value.byteLength;
        if (!buffer || required > buffer.byteLength) {
          const capacity = Math.min(
            limit,
            Math.max(64 * 1024, required, (buffer?.byteLength ?? 0) * 2),
          );
          const grown = new Uint8Array(capacity);
          if (buffer) grown.set(buffer.subarray(0, bytes));
          buffer = grown;
        }
        buffer.set(value, bytes);
        bytes += value.byteLength;
        if (stopOnTerminal && response.ok) {
          if (consumeTerminalFrames(streamDecoder.decode(value, { stream: true }))) break;
        }
      }
    }
    if (controller.signal.aborted) throw controller.signal.reason;
    return {
      status: response.status,
      ok: response.ok,
      text: buffer ? new TextDecoder().decode(buffer.subarray(0, bytes)) : "",
      pageUrl: location.href,
      title: document.title,
    };
  } catch (error) {
    const timedOut = controller.signal.aborted;
    const tooLarge = error?.code === "producer_browser_response_too_large";
    const kind = stopOnTerminal ? "stream" : "fetch";
    return {
      status: timedOut ? 504 : 502,
      ok: false,
      text: timedOut ? "Producer browser request timed out."
        : tooLarge ? "Producer browser response exceeds 16 MiB."
        : "Producer browser transport failed.",
      pageUrl: location.href,
      title: document.title,
      fetchError: timedOut ? `producer_browser_${kind}_timeout`
        : tooLarge ? "producer_browser_response_too_large"
        : `producer_browser_${kind}_failed`,
    };
  } finally {
    clearTimeout(timeoutId);
    controller.signal.removeEventListener("abort", cancel);
    if (reader) {
      await reader.cancel().catch(() => undefined);
      reader.releaseLock();
    }
  }
}
