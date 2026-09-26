import { EventEmitter } from "node:events";
import { createProgramCdpSessions } from "../gemini-canvas-program-handle-cdp-sessions.mjs";
import { turn } from "./gemini-canvas-program-handle.response-fixtures.mjs";

// Model public CDP envelopes, including the automatic/direct page-session handoff.
export class ProtocolRoot extends EventEmitter {
  constructor(respond) {
    super();
    this.respond = respond;
    this.commands = [];
    this.targets = new Map();
    this.detached = 0;
  }
  emitTo(route, message) {
    for (const sessionId of [...route].reverse()) {
      message = { method: "Target.receivedMessageFromTarget", params: { sessionId, message: JSON.stringify(message) } };
    }
    this.emit(message.method, message.params);
  }
  attach(route, sessionId, type = "worker", targetId = sessionId, browserContextId = "owned") {
    const targetInfo = { type, targetId, browserContextId };
    this.targets.set(targetId, targetInfo);
    this.emitTo(route, { method: "Target.attachedToTarget", params: { sessionId, targetInfo, waitingForDebugger: true } });
  }
  async dispatch(route, method, params) {
    this.commands.push({ route, method, params });
    if (method === "Target.sendMessageToTarget") {
      const message = JSON.parse(params.message);
      const childRoute = [...route, params.sessionId];
      void this.dispatch(childRoute, message.method, message.params).then(
        (result) => this.emitTo(childRoute, { id: message.id, result }),
        () => this.emitTo(childRoute, { id: message.id, error: { message: "private fixture diagnostic" } }),
      );
      return {};
    }
    if (!route.length && method === "Target.setAutoAttach") this.attach([], "auto-initial", "page", "initial");
    if (!route.length && method === "Target.attachToTarget") {
      const sessionId = `direct-${params.targetId}`;
      this.emitTo([], { method: "Target.attachedToTarget", params: { sessionId, targetInfo: this.targets.get(params.targetId) } });
      return { sessionId };
    }
    if (method === "Target.detachFromTarget") this.emitTo(route, { method: "Target.detachedFromTarget", params });
    return this.respond ? this.respond({ route, method, params }) : { route };
  }
  send(method, params) { return this.dispatch([], method, params); }
  async detach() { this.detached++; this.emit("close"); }
}

export async function cdpFixture(t, options = {}) {
  const root = new ProtocolRoot(options.respond);
  const identity = { detached: 0, async send() { return { targetInfo: { browserContextId: "owned" } }; },
    async detach() { this.detached++; } };
  const configured = [], disposed = [], failures = [];
  let browserRequested = false;
  const context = {
    newCDPSession: options.newCDPSession ?? (async () => identity),
    browser: () => ({ newBrowserCDPSession: () => {
      browserRequested = true;
      return options.newBrowserCDPSession?.() ?? Promise.resolve(root);
    } }),
  };
  const owner = createProgramCdpSessions({ context: () => context }, {
    timeoutMs: options.timeoutMs,
    configure: async (session, signal) => {
      configured.push(session);
      const listener = () => undefined;
      session.on("Fixture.network", listener);
      let revoked = false;
      const dispose = () => {
        if (revoked) return;
        revoked = true;
        signal.removeEventListener("abort", dispose);
        session.off("Fixture.network", listener);
        disposed.push(session);
      };
      signal.addEventListener("abort", dispose, { once: true });
      await options.configure?.(session, root);
      return dispose;
    },
    onError: (error) => failures.push(error),
  });
  t.after(() => owner.stop());
  if (!options.deferReady) await owner.ready;
  return { root, identity, owner, configured, disposed, failures, browserRequested: () => browserRequested };
}

export async function until(predicate) {
  for (let attempt = 0; attempt < 40; attempt++) {
    if (predicate()) return;
    await turn();
  }
  throw new Error("Fixture condition did not settle.");
}
