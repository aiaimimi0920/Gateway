import { readNodeResponseText } from "./response-body.mjs";
import { executeBrowserTransport } from "./browser-transport.mjs";

export async function browserContextFetch(page, url, options, timeoutMs = 60_000) {
  return await page.evaluate(executeBrowserTransport, {
    url, options, timeoutMs, stopOnTerminal: false,
  });
}

export async function browserContextReadSse(page, url, options, timeoutMs = 60_000) {
  return await page.evaluate(executeBrowserTransport, {
    url, options, timeoutMs, stopOnTerminal: true,
  });
}

export async function nodeSideFetch(url, options = {}, timeoutMs = 60_000) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
  try {
    const response = await fetch(url, {
      ...options,
      redirect: "error",
      signal: controller.signal,
    });
    const text = await readNodeResponseText(response, controller.signal);
    return {
      status: response.status,
      ok: response.ok,
      text,
    };
  } catch (error) {
    const isTimeout = controller.signal.aborted || error?.name === "AbortError";
    const tooLarge = error?.code === "producer_node_response_too_large";
    return {
      status: isTimeout ? 504 : 502,
      ok: false,
      text: isTimeout ? "Producer request timed out." : tooLarge ? "Producer response exceeds 16 MiB." : "Producer request failed.",
      fetchError: isTimeout ? "producer_node_fetch_timeout" : tooLarge ? "producer_node_response_too_large" : "producer_node_fetch_failed",
    };
  } finally {
    clearTimeout(timeoutId);
  }
}
