import assert from "node:assert/strict";
import vm from "node:vm";

export const plain = (value) => JSON.parse(JSON.stringify(value));
export const turn = () => new Promise((resolve) => setImmediate(resolve));

export function executorFixture(options = {}) {
  const timers = new Map(), sockets = [], requests = [], inputs = [];
  let sequence = 0;
  class Socket {
    constructor(url) {
      if (options.constructorError) throw options.constructorError;
      this.url = url; this.sent = []; this.listeners = new Map(); this.closeCalls = 0;
      sockets.push(this);
    }
    addEventListener(type, callback) {
      const listeners = this.listeners.get(type) ?? [];
      listeners.push(callback); this.listeners.set(type, listeners);
    }
    send(value) { this.sent.push(JSON.parse(value)); }
    close() {
      this.closeCalls += 1;
      if (options.closeError) throw options.closeError;
      for (const callback of this.listeners.get("close") ?? []) callback({ code: 1000, reason: "fixture close" });
    }
    async emit(type, event = {}) {
      await Promise.all((this.listeners.get(type) ?? []).map((callback) => callback(event)));
    }
    async message(value) { await this.emit("message", { data: JSON.stringify(value) }); }
  }
  const sandbox = vm.createContext({ URL, AbortController, DOMException, Error, Uint8Array, TextDecoder, btoa, atob: options.atob ?? atob,
    WebSocket: Socket,
    setTimeout: (callback, ms) => { const id = ++sequence; timers.set(id, { callback, ms }); return id; },
    clearTimeout: (id) => timers.delete(id),
    fetch: (...args) => {
      requests.push(args);
      assert.ok(options.fetch, "Every fetch must be explicitly synthetic");
      return options.fetch(...args);
    },
  });
  const target = { evaluate: async (callback, input) => {
    const preparedInput = options.prepareEvaluateInput ? options.prepareEvaluateInput(input) : input;
    inputs.push(structuredClone(preparedInput)); sandbox.input = inputs.at(-1);
    // Serialization discards the Node module closure, matching browser evaluate.
    return await vm.runInContext(`(${callback.toString()})(input)`, sandbox);
  } };
  const fire = async (ms) => {
    const timer = [...timers].find(([, value]) => value.ms === ms);
    assert.ok(timer, `Expected an active ${ms} ms deadline`);
    timers.delete(timer[0]); timer[1].callback(); await turn();
  };
  return { target, timers, sockets, requests, inputs, fire };
}
