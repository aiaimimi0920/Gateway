import { afterEach, describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../api/console";
import { GatewayApiError } from "../api/errors";
import { authenticatedSession, createApi } from "./managementSessionTestFixtures";
import { rotateManagementToken } from "./managementTokenRotation";

describe("bounded management rotation readback", () => {
  afterEach(() => { vi.useRealTimers(); });

  it.each(["", "中文密钥", "with space", "line\r\nbreak", "x".repeat(4097)])("rejects an unusable replacement before any write", async value => {
    const api = createApi();
    await expect(rotateManagementToken(api, "old", value, () => true)).rejects.toThrow("visible ASCII");
    expect(api.rotateSession).not.toHaveBeenCalled();
    expect(api.verifySession).not.toHaveBeenCalled();
  });

  it.each([400, 401, 403, 409, 429])("does not probe or repeat a rejected HTTP %s rotation", async status => {
    const failure = new GatewayApiError("Rejected", status);
    const api = createApi({ rotateSession: vi.fn().mockRejectedValue(failure) });
    await expect(rotateManagementToken(api, "old", "new", () => true)).rejects.toBe(failure);
    expect(api.rotateSession).toHaveBeenCalledOnce();
    expect(api.verifySession).not.toHaveBeenCalled();
  });

  it("verifies exactly once after an ambiguous server error", async () => {
    const api = createApi({ rotateSession: vi.fn().mockRejectedValue(new GatewayApiError("Upstream lost", 502)) });
    await expect(rotateManagementToken(api, "old", "new", () => true)).resolves.toEqual(authenticatedSession);
    expect(api.rotateSession).toHaveBeenCalledOnce();
    expect(api.verifySession).toHaveBeenCalledOnce();
  });

  it("preserves the original error when the candidate cannot be verified", async () => {
    const failure = new TypeError("Rotation response lost");
    const api = createApi({
      rotateSession: vi.fn().mockRejectedValue(failure),
      verifySession: vi.fn().mockRejectedValue(new GatewayApiError("Candidate invalid", 401)),
    });
    await expect(rotateManagementToken(api, "old", "new", () => true)).rejects.toBe(failure);
    expect(api.verifySession).toHaveBeenCalledOnce();
  });

  it("does not send a candidate after the operation becomes obsolete", async () => {
    const failure = new TypeError("Rotation response lost");
    const api = createApi({ rotateSession: vi.fn().mockRejectedValue(failure) });
    await expect(rotateManagementToken(api, "old", "new", () => false)).rejects.toBe(failure);
    expect(api.verifySession).not.toHaveBeenCalled();
  });

  it("aborts readback after five seconds and clears the timer", async () => {
    vi.useFakeTimers();
    const failure = new TypeError("Rotation response lost");
    let signal: AbortSignal | undefined;
    const api = createApi({
      rotateSession: vi.fn().mockRejectedValue(failure),
      verifySession: vi.fn<ConsoleApi["verifySession"]>((_token, options) => new Promise((_resolve, reject) => {
        signal = options?.signal ?? undefined;
        signal?.addEventListener("abort", () => reject(new Error("aborted")), { once: true });
      })),
    });
    const result = rotateManagementToken(api, "old", "new", () => true).catch(error => error);
    await vi.advanceTimersByTimeAsync(5000);
    expect(await result).toBe(failure);
    expect(signal?.aborted).toBe(true);
    expect(vi.getTimerCount()).toBe(0);
    expect(api.rotateSession).toHaveBeenCalledOnce();
  });
});
