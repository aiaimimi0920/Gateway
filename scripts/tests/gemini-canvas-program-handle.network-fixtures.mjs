import { EventEmitter } from "node:events";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";
import { createProgramNetworkCaptureOwner } from "../gemini-canvas-program-handle-network-capture.mjs";
import { requestFixture as poolRequestFixture, dispatch } from "./gemini-canvas-browser-pool.network-fixtures.mjs";

export { responseFixture, deferred, turn, streamUrl, rpcUrl, pairText } from "./gemini-canvas-browser-pool.network-fixtures.mjs";

export function requestFixture(options) {
  return { ...poolRequestFixture(options), resourceType: () => "fetch" };
}

// Parser/retention tests inject responses; native admission has protocol tests.
export function fixtureResponseCapture(page, { onResponse, onWebSocket }) {
  const context = page.context();
  context.on("response", onResponse);
  context.on("websocket", onWebSocket);
  return {
    ready: Promise.resolve(),
    waitForTargets: async () => undefined,
    stop() { context.off("response", onResponse); context.off("websocket", onWebSocket); },
  };
}

export async function networkFixture(t, operation = "text", options = {}) {
  const app = await importTestableProgramHandle({ operation });
  const page = options.page ?? new EventEmitter();
  page.cookies ??= options.cookies ?? (async () => []);
  page.context ??= () => page;
  const owner = createProgramNetworkCaptureOwner({ ...app, operation, captureBudgetLimits: options.limits,
    createResponseCapture: fixtureResponseCapture });
  const capture = owner.startNetworkCapture(page, options.state);
  t.after(() => capture.stop());
  return { page, capture, state: capture.state, emit: (event, value) => dispatch(page, event, value) };
}
