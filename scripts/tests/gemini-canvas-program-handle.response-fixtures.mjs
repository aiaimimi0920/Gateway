import { EventEmitter } from "node:events";
import { createProgramResponseCapture } from "../gemini-canvas-program-handle-response-capture.mjs";

export const turn = () => new Promise((resolve) => setImmediate(resolve));
export function deferred() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

export class NativeSession extends EventEmitter {
  constructor(options = {}) {
    super();
    this.options = options;
    this.commands = [];
    this.detached = 0;
    this.bodies = new Map();
  }
  async send(method, params) {
    this.commands.push({ method, params });
    if (method === "Network.enable" && this.options.enable) return this.options.enable();
    if (method === "Network.getResponseBody") {
      const body = this.bodies.get(params.requestId) ?? { body: "small", base64Encoded: false };
      return typeof body === "function" ? body() : body;
    }
    return {};
  }
  async detach() { this.detached++; }
}

export async function responseFixture(t, options = {}) {
  const session = new NativeSession(options);
  const rows = [], failures = [];
  const page = {};
  const createSessions = options.createSessions ?? ((_page, { configure, onError }) => {
    const controller = new AbortController();
    let stopping = null;
    const stop = () => {
      if (!stopping) { controller.abort(); stopping = session.detach(); }
      return stopping;
    };
    const ready = configure(session, controller.signal).catch((error) => { onError(error); throw error; });
    void ready.catch(() => undefined);
    return { ready, stop };
  });
  const capture = createProgramResponseCapture(page, {
    createSessions,
    bodyBytes: options.bodyBytes ?? 16,
    binaryBodyBytes: options.binaryBodyBytes ?? options.bodyBytes ?? 16,
    requestFilter: (url) => url.includes("/app/"),
    onResponse: (response) => {
      const row = { url: response.url(), status: response.status(), method: response.request().method(),
        contentType: response.headers()["content-type"], response,
        promise: options.readText === false ? null : response.text() };
      rows.push(row);
      return options.onResponse?.(response);
    },
    onError: (error) => failures.push(error),
  });
  t.after(() => capture.stop());
  if (!options.deferReady) await capture.ready;
  const request = (id = "one", method = "POST", url = "https://fixture.invalid/app/request") =>
    session.emit("Network.requestWillBeSent", { requestId: id, request: { url, method } });
  const response = (id = "one", status = 200, headers = { "Content-Type": "text/plain" }) =>
    session.emit("Network.responseReceived", { requestId: id,
      response: { url: "https://fixture.invalid/app/request", status, headers } });
  const bytes = (length, id = "one") => session.emit("Network.dataReceived", { requestId: id, dataLength: length });
  const finish = (id = "one") => session.emit("Network.loadingFinished", { requestId: id });
  const bodyCalls = () => session.commands.filter((command) => command.method === "Network.getResponseBody");
  return { session, capture, rows, failures, request, response, bytes, finish, bodyCalls };
}
