import { EventEmitter } from "node:events";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

export const app = await importTestableScript();
export const rpcUrl = "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=MaZiqc";
export const streamUrl = "https://gemini.google.com/StreamGenerate";
export const pairText = '"c_deadbeef", "r_cafebabe"';
export const turn = () => new Promise((resolve) => setImmediate(resolve));

export function deferred() {
  let resolve, reject;
  const promise = new Promise((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}

export function requestFixture({ url = rpcUrl, method = "POST", data = "", headers = {} } = {}) {
  return { url: () => url, method: () => method, postData: () => data, headers: () => headers };
}

export function responseFixture({ request = requestFixture(), mime = "application/json", headers = {}, text = async () => "", body = async () => Buffer.from([1, 2, 3]) } = {}) {
  return { url: request.url, request: () => request, status: () => 200, headers: () => ({ "content-type": mime, ...headers }), text, body };
}

export async function dispatch(page, event, value) {
  for (const listener of page.listeners(event)) await listener(value);
}

export function networkFixture(t, operation = "bootstrap_program", options = {}) {
  const page = options.page ?? new EventEmitter();
  page.context ??= () => ({ cookies: options.cookies ?? (async () => []) });
  const capture = app.startNetworkCapture(page, operation, options.state);
  t.after(() => capture.stop());
  return { page, capture, state: capture.state, emit: (event, value) => dispatch(page, event, value) };
}
