import { describe, expect, it, vi } from "vitest";

import type { ConsoleApi } from "../../api/console";
import { ConsoleRefreshSingleFlight } from "./consoleRefreshSingleFlight";

function deferred(): {
  promise: Promise<void>;
  resolve: () => void;
  reject: (cause: unknown) => void;
} {
  let resolve!: () => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<void>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

describe("ConsoleRefreshSingleFlight", () => {
  it("shares one active refresh for the same API session", async () => {
    const gate = new ConsoleRefreshSingleFlight();
    const api = {} as ConsoleApi;
    const pending = deferred();
    const operation = vi.fn(() => pending.promise);

    const first = gate.run(api, "token-a", operation);
    const second = gate.run(api, "token-a", operation);

    expect(second).toBe(first);
    expect(operation).not.toHaveBeenCalled();
    await Promise.resolve();
    expect(operation).toHaveBeenCalledTimes(1);
    pending.resolve();
    await first;
  });

  it("starts a new refresh for a different session and after settlement", async () => {
    const gate = new ConsoleRefreshSingleFlight();
    const api = {} as ConsoleApi;
    const firstPending = deferred();
    const secondPending = deferred();
    const firstOperation = vi.fn(() => firstPending.promise);
    const secondOperation = vi.fn(() => secondPending.promise);

    const first = gate.run(api, "token-a", firstOperation);
    const second = gate.run(api, "token-b", secondOperation);
    expect(second).not.toBe(first);
    await Promise.resolve();
    expect(firstOperation).toHaveBeenCalledTimes(1);
    expect(secondOperation).toHaveBeenCalledTimes(1);

    firstPending.resolve();
    secondPending.resolve();
    await Promise.all([first, second]);

    const next = gate.run(api, "token-b", vi.fn(async () => undefined));
    expect(next).not.toBe(second);
    await next;
  });

  it("clears a rejected refresh so it can be retried", async () => {
    const gate = new ConsoleRefreshSingleFlight();
    const api = {} as ConsoleApi;
    const failed = gate.run(api, "token-a", async () => {
      throw new Error("refresh failed");
    });

    await expect(failed).rejects.toThrow("refresh failed");
    const retry = vi.fn(async () => undefined);
    await gate.run(api, "token-a", retry);
    expect(retry).toHaveBeenCalledTimes(1);
  });
});
