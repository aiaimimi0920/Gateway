import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { authenticatedSession, createApi, deferred, Providers } from "./managementSessionTestFixtures";
import type { ManagementSession } from "../api/contracts";
import { createBrowserHost } from "../platform/browserHost";
import { readManagementSessionToken, writeManagementSessionToken } from "./storage";
import { useManagementSession } from "./useManagementSession";

describe("management token rotation recovery", () => {
  beforeEach(() => { window.sessionStorage.clear(); window.localStorage.clear(); });

  it("recovers a committed rotation after its response is lost without rotating twice", async () => {
    writeManagementSessionToken("old-token");
    const api = createApi({ rotateSession: vi.fn().mockRejectedValue(new TypeError("Failed to fetch")) });
    const { result } = renderHook(useManagementSession, {
      wrapper: ({ children }) => <Providers api={api}>{children}</Providers>,
    });
    await waitFor(() => expect(result.current.phase).toBe("authenticated"));
    await act(async () => { await result.current.confirmSecretAccess("old-token"); });
    let failure: unknown;
    await act(async () => { await result.current.rotate("new-token").catch(error => { failure = error; }); });
    expect(failure).toBeUndefined();
    expect(api.rotateSession).toHaveBeenCalledExactlyOnceWith("old-token", "new-token");
    expect(api.verifySession).toHaveBeenLastCalledWith("new-token", expect.objectContaining({ notifyAuthenticationFailure: false }));
    expect(result.current.managementToken).toBe("new-token");
    expect(result.current.session).toEqual({ ...authenticatedSession, secretAccessGranted: true });
    expect(api.confirmSecretAccess).toHaveBeenLastCalledWith("new-token", "new-token");
    expect(result.current.secretGrant?.grant).toBe("short-lived-grant");
    expect(readManagementSessionToken()).toBe("new-token");
    expect(window.localStorage.length).toBe(0);
  });

  it("does not resurrect a session when logout races with candidate verification", async () => {
    writeManagementSessionToken("old-token");
    const candidate = deferred<ManagementSession>();
    const api = createApi({
      rotateSession: vi.fn().mockRejectedValue(new TypeError("response lost")),
      verifySession: vi.fn().mockResolvedValueOnce(authenticatedSession).mockImplementation(() => candidate.promise),
    });
    const { result } = renderHook(useManagementSession, {
      wrapper: ({ children }) => <Providers api={api}>{children}</Providers>,
    });
    await waitFor(() => expect(result.current.phase).toBe("authenticated"));
    let rotation!: Promise<void>;
    await act(async () => { rotation = result.current.rotate("new-token"); });
    await act(async () => { await result.current.logout(); });
    await act(async () => { candidate.resolve(authenticatedSession); await rotation; });
    expect(result.current.phase).toBe("unauthenticated");
    expect(result.current.managementToken).toBeNull();
    expect(readManagementSessionToken()).toBeNull();
  });

  it("does not restore an earlier server after switching hosts during readback", async () => {
    writeManagementSessionToken("old-token");
    const candidate = deferred<ManagementSession>();
    const api = createApi({
      rotateSession: vi.fn().mockRejectedValue(new TypeError("response lost")),
      verifySession: vi.fn().mockResolvedValueOnce(authenticatedSession).mockImplementation(() => candidate.promise),
    });
    let adapter = createBrowserHost();
    const { result, rerender } = renderHook(useManagementSession, {
      wrapper: ({ children }) => <Providers api={api} adapter={adapter}>{children}</Providers>,
    });
    await waitFor(() => expect(result.current.phase).toBe("authenticated"));
    let rotation!: Promise<void>;
    await act(async () => { rotation = result.current.rotate("new-token"); });
    adapter = createBrowserHost({ origin: "https://second.example" });
    rerender();
    await waitFor(() => expect(result.current.phase).toBe("unauthenticated"));
    await act(async () => { candidate.resolve(authenticatedSession); await rotation; });
    expect(result.current.phase).toBe("unauthenticated");
    expect(result.current.managementToken).toBeNull();
    expect(JSON.stringify(window.sessionStorage)).not.toContain("new-token");
  });
});
