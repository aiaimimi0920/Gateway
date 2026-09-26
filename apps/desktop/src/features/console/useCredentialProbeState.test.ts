import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { GatewayApiError } from "../../api/errors";
import { useCredentialProbeState } from "./useCredentialProbeState";

describe("credential probe secret recovery", () => {
  it.each([
    { code: "forbidden", current: true, expected: "not-required" },
    { code: "console_secret_access_required", current: false, expected: "stale" },
    { code: "console_secret_access_required", current: true, expected: "recovered" },
  ])("handles $code with current=$current", ({ code, current, expected }) => {
    const clearSecretGrant = vi.fn();
    const setSecretDialogOpen = vi.fn();
    const { result } = renderHook(() => useCredentialProbeState({ clearSecretGrant, setSecretDialogOpen }));
    act(() => {
      result.current.setCredentialProbeBusy("account");
      result.current.setCredentialProbeResults({ account: {
        credentialId: "account", providerId: "provider", probePoint: "chat",
        status: "error", message: "old result", checkedAt: "2026-09-09T00:00:00Z",
      } });
    });
    const generation = result.current.credentialProbeGenerationRef.current;
    let outcome: string | undefined;
    act(() => {
      outcome = result.current.handleSecretAccessRequiredError(
        new GatewayApiError("permission required", 403, code), () => current,
      );
    });
    expect(outcome).toBe(expected);
    if (expected === "recovered") {
      expect(clearSecretGrant).toHaveBeenCalledOnce();
      expect(setSecretDialogOpen).toHaveBeenCalledWith(true);
      expect(result.current.credentialProbeGenerationRef.current).toBe(generation + 1);
      expect(result.current.credentialProbeBusy).toBeNull();
      expect(result.current.credentialProbeResults).toEqual({});
    } else {
      expect(clearSecretGrant).not.toHaveBeenCalled();
      expect(setSecretDialogOpen).not.toHaveBeenCalled();
      expect(result.current.credentialProbeGenerationRef.current).toBe(generation);
      expect(result.current.credentialProbeBusy).toBe("account");
      expect(result.current.credentialProbeResults.account.message).toBe("old result");
    }
  });
});
