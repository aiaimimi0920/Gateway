import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { writeManagementSessionToken } from "./storage";
import { createApi, deferred, Providers, SessionHarness } from "./managementSessionTestFixtures";
import type { SecretGrant } from "../api/contracts";

const grant = (value: string, ttl: number): SecretGrant => ({ grant: value, expiresAt: new Date(Date.now() + ttl).toISOString() });
async function mount(api: ReturnType<typeof createApi>) {
  writeManagementSessionToken("stored-token");
  render(<Providers api={api}><SessionHarness /></Providers>);
  await waitFor(() => expect(screen.getByTestId("secret-access")).toHaveTextContent("true"));
}
describe("ManagementSessionProvider automatic secret access", () => {
  beforeEach(() => { window.localStorage.clear(); window.sessionStorage.clear(); });

  it("renews an in-memory grant before expiry without another password", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const api = createApi({ confirmSecretAccess: vi.fn().mockResolvedValueOnce(grant("first", 10_000))
        .mockImplementation(async () => grant("renewed", 600_000)) });
      await mount(api);
      await act(async () => { await vi.advanceTimersByTimeAsync(9_000); });
      expect(api.confirmSecretAccess).toHaveBeenCalledTimes(2);
      expect(api.confirmSecretAccess).toHaveBeenLastCalledWith("stored-token", "stored-token");
      expect(screen.getByTestId("grant")).toHaveTextContent("renewed");
      await act(async () => { await vi.advanceTimersByTimeAsync(1_001); });
      expect(screen.getByTestId("secret-access")).toHaveTextContent("true");
      expect(JSON.stringify(window.sessionStorage)).not.toContain("renewed");
    } finally { vi.useRealTimers(); }
  });

  it("clears expired access even when renewal fails and retries at a bounded interval", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const api = createApi({ confirmSecretAccess: vi.fn().mockResolvedValueOnce(grant("first", 10_000))
        .mockRejectedValue(new Error("offline")) });
      await mount(api);
      await act(async () => { await vi.advanceTimersByTimeAsync(10_001); });
      expect(screen.getByTestId("grant")).toHaveTextContent("none");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
      expect(api.confirmSecretAccess).toHaveBeenCalledTimes(2);
      await act(async () => { await vi.advanceTimersByTimeAsync(28_000); });
      expect(api.confirmSecretAccess).toHaveBeenCalledTimes(2);
      await act(async () => { await vi.advanceTimersByTimeAsync(1_000); });
      expect(api.confirmSecretAccess).toHaveBeenCalledTimes(3);
    } finally { vi.useRealTimers(); }
  });

  it("reschedules expiry beyond the browser timer limit and retains a single stalled renewal", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const pending = deferred<SecretGrant>();
      const api = createApi({ confirmSecretAccess: vi.fn().mockResolvedValueOnce(grant("long", 2_147_483_647 + 31_000))
        .mockReturnValue(pending.promise) });
      await mount(api);
      await act(async () => { await vi.advanceTimersByTimeAsync(2_147_483_647); });
      expect(screen.getByTestId("grant")).toHaveTextContent("long");
      expect(api.confirmSecretAccess).toHaveBeenCalledOnce();
      await act(async () => { await vi.advanceTimersByTimeAsync(31_001); });
      expect(screen.getByTestId("grant")).toHaveTextContent("none");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
      expect(api.confirmSecretAccess).toHaveBeenCalledTimes(2);
      await act(async () => { pending.resolve(grant("recovered", 600_000)); });
      expect(screen.getByTestId("grant")).toHaveTextContent("recovered");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("true");
    } finally { vi.useRealTimers(); }
  });
});
