import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../api/console";
import type { ManagementSession } from "../api/contracts";
import { GatewayApiError } from "../api/errors";
import { AuthBoundary } from "../features/auth/AuthBoundary";
import { HostProvider } from "../platform/HostProvider";
import { createBrowserHost } from "../platform/browserHost";
import { createTauriHost } from "../platform/tauriHost";
import type { GatewayHostAdapter } from "../platform/types";
import { readManagementSessionToken, writeManagementSessionToken } from "./storage";
import { ManagementSessionProvider } from "./ManagementSessionProvider";
import { useManagementSession } from "./useManagementSession";

const authenticatedSession: ManagementSession = {
  role: "administrator",
  capabilities: ["route-config:read", "route-config:write"],
  activeRevision: "r1-deadbeef",
  secretAccessGranted: false,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function createApi(overrides: Partial<ConsoleApi> = {}): ConsoleApi {
  return {
    getBootstrapStatus: vi.fn().mockResolvedValue({
      needsBootstrap: false,
      managementConfigured: true,
      environmentOverride: false,
    }),
    bootstrap: vi.fn().mockResolvedValue({ success: true }),
    verifySession: vi.fn().mockResolvedValue(authenticatedSession),
    confirmSecretAccess: vi.fn().mockResolvedValue({
      grant: "short-lived-grant",
      expiresAt: "2099-01-01T00:00:00Z",
    }),
    rotateSession: vi.fn().mockResolvedValue({ success: true }),
    logout: vi.fn().mockResolvedValue({ success: true }),
    probeCredential: vi.fn().mockResolvedValue({
      result: {
        credentialId: "credential-1",
        providerId: "provider-1",
        status: "unsupported",
        message: "Credential probe is unavailable in this test.",
        checkedAt: "2099-01-01T00:00:00Z",
      },
    }),
    getAccountGroupSummary: vi.fn().mockRejectedValue(new Error("summary unavailable")),
    getRouteConfig: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeef", sequence: 1 },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        mutationSupported: true,
      },
    }),
    validateRouteConfig: vi.fn().mockResolvedValue({
      validation: {
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
      },
    }),
    commitRouteConfig: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r2-beadfeed", sequence: 2 },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        mutationSupported: true,
      },
      committed: true,
    }),
    getRouteConfigRevision: vi.fn().mockResolvedValue({
      routeConfig: {
        revision: { id: "r1-deadbeef", sequence: 1 },
        source: "redis",
        diagnostics: { diagnostics: [] },
        requiresRepair: false,
        document: { providers: [], model_routes: [], aliases: {} },
        secrets: [],
        mutationSupported: true,
      },
      active: true,
      hasArchive: true,
    }),
    listRouteConfigRevisions: vi.fn().mockResolvedValue({
      revisions: [],
    }),
    ...overrides,
  };
}

function Providers({
  api,
  children,
  adapter = createBrowserHost(),
}: {
  api: ConsoleApi;
  children: ReactNode;
  adapter?: GatewayHostAdapter;
}) {
  return (
    <HostProvider adapter={adapter}>
      <ManagementSessionProvider api={api}>{children}</ManagementSessionProvider>
    </HostProvider>
  );
}

function SessionHarness() {
  const session = useManagementSession();
  return (
    <div>
      <output data-testid="phase">{session.phase}</output>
      <output data-testid="grant">{session.secretGrant?.grant ?? "none"}</output>
      <output data-testid="secret-access">
        {String(session.session?.secretAccessGranted ?? false)}
      </output>
      <output data-testid="management-token">{session.managementToken ?? "none"}</output>
      <output data-testid="session-error">{session.error ?? "none"}</output>
      <button
        type="button"
        onClick={() => void session.login("profile-one-token").catch(() => undefined)}
      >
        Log in
      </button>
      <button type="button" onClick={() => void session.confirmSecretAccess("management-secret")}>
        Confirm secret
      </button>
      <button type="button" onClick={session.clearSecretGrant}>
        Clear secret
      </button>
      <button type="button" onClick={() => void session.rotate("replacement-token")}>
        Rotate
      </button>
      <button type="button" onClick={() => void session.logout()}>
        Log out
      </button>
    </div>
  );
}

describe("ManagementSessionProvider", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("checks bootstrap status before presenting login", async () => {
    let resolveStatus:
      | ((value: {
          needsBootstrap: false;
          managementConfigured: true;
          environmentOverride: false;
        }) => void)
      | undefined;
    const getBootstrapStatus = vi.fn(
      () =>
        new Promise<{
          needsBootstrap: false;
          managementConfigured: true;
          environmentOverride: false;
        }>((resolve) => {
          resolveStatus = resolve;
        }),
    );
    const api = createApi({ getBootstrapStatus });

    render(
      <Providers api={api}>
        <AuthBoundary>
          <div>Console ready</div>
        </AuthBoundary>
      </Providers>,
    );

    expect(screen.getByRole("status")).toHaveTextContent(/正在检查 Gateway 管理员初始化状态/i);
    expect(screen.queryByRole("heading", { name: /登录/i })).not.toBeInTheDocument();
    expect(getBootstrapStatus).toHaveBeenCalledOnce();

    await act(async () =>
      resolveStatus?.({
        needsBootstrap: false,
        managementConfigured: true,
        environmentOverride: false,
      }),
    );

    expect(await screen.findByRole("heading", { name: /登录/i })).toBeInTheDocument();
  });

  it("restores a versioned session only after bootstrap status", async () => {
    const calls: string[] = [];
    writeManagementSessionToken("stored-token");
    const api = createApi({
      getBootstrapStatus: vi.fn(async () => {
        calls.push("bootstrap-status");
        return {
          needsBootstrap: false,
          managementConfigured: true,
          environmentOverride: false,
        };
      }),
      verifySession: vi.fn(async (token) => {
        calls.push(`verify:${token}`);
        return authenticatedSession;
      }),
    });

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    expect(calls).toEqual(["bootstrap-status", "verify:stored-token"]);
  });

  it("isolates persisted and in-memory sessions when the Tauri API origin changes", async () => {
    const api = createApi();
    const user = userEvent.setup();
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    await user.click(screen.getByRole("button", { name: /^log in$/i }));
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    await user.click(screen.getByRole("button", { name: /confirm secret/i }));
    expect(await screen.findByTestId("grant")).toHaveTextContent("short-lived-grant");
    expect(screen.getByTestId("management-token")).toHaveTextContent("profile-one-token");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("true");

    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );

    expect(screen.getByTestId("management-token")).toHaveTextContent("none");
    expect(screen.getByTestId("grant")).toHaveTextContent("none");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    expect(api.verifySession).toHaveBeenCalledTimes(1);
    expect(api.verifySession).toHaveBeenLastCalledWith("profile-one-token");

    view.rerender(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    expect(screen.getByTestId("management-token")).toHaveTextContent("profile-one-token");
    expect(api.verifySession).toHaveBeenCalledTimes(2);
    expect(api.verifySession).toHaveBeenLastCalledWith("profile-one-token");
  });

  it("ignores a pending login result from the previous Tauri API origin", async () => {
    const verification = deferred<ManagementSession>();
    const api = createApi({ verifySession: vi.fn(() => verification.promise) });
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    fireEvent.click(screen.getByRole("button", { name: /^log in$/i }));

    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));

    await act(async () => verification.resolve(authenticatedSession));

    expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated");
    expect(screen.getByTestId("management-token")).toHaveTextContent("none");
    expect(screen.getByTestId("grant")).toHaveTextContent("none");
  });

  it("ignores a pending authentication failure from the previous Tauri API origin", async () => {
    const verification = deferred<ManagementSession>();
    const api = createApi({ verifySession: vi.fn(() => verification.promise) });
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    fireEvent.click(screen.getByRole("button", { name: /^log in$/i }));
    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));

    await act(async () =>
      verification.reject(
        new GatewayApiError("Expired token", 401, "console_management_token_invalid"),
      ),
    );

    expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated");
    expect(screen.getByTestId("session-error")).toHaveTextContent("none");
  });

  it("ignores a pending secret grant from the previous Tauri API origin", async () => {
    const confirmation = deferred<{ grant: string; expiresAt: string }>();
    const api = createApi({ confirmSecretAccess: vi.fn(() => confirmation.promise) });
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    fireEvent.click(screen.getByRole("button", { name: /^log in$/i }));
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    fireEvent.click(screen.getByRole("button", { name: /confirm secret/i }));

    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));

    await act(async () =>
      confirmation.resolve({
        grant: "stale-host-grant",
        expiresAt: "2099-01-01T00:00:00Z",
      }),
    );

    expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated");
    expect(screen.getByTestId("grant")).toHaveTextContent("none");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
  });

  it("keeps secret grants in memory and out of browser storage", async () => {
    writeManagementSessionToken("stored-token");
    const api = createApi();
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /confirm secret/i }));

    expect(await screen.findByTestId("grant")).toHaveTextContent("short-lived-grant");
    expect(window.sessionStorage.length).toBe(1);
    expect(JSON.stringify(window.sessionStorage)).not.toContain("short-lived-grant");
    expect(window.localStorage.length).toBe(0);
  });

  it("clears both the in-memory grant and the session access flag", async () => {
    writeManagementSessionToken("stored-token");
    const api = createApi();
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /confirm secret/i }));
    expect(await screen.findByTestId("secret-access")).toHaveTextContent("true");

    await user.click(screen.getByRole("button", { name: /clear secret/i }));

    expect(screen.getByTestId("grant")).toHaveTextContent("none");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
  });

  it("updates storage only after token rotation succeeds", async () => {
    writeManagementSessionToken("stored-token");
    let resolveRotate: ((value: { success: true }) => void) | undefined;
    const rotateSession = vi.fn(
      () =>
        new Promise<{ success: true }>((resolve) => {
          resolveRotate = resolve;
        }),
    );
    const api = createApi({ rotateSession });
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /rotate/i }));
    expect(readManagementSessionToken()).toBe("stored-token");

    await act(async () => resolveRotate?.({ success: true }));
    await waitFor(() => expect(readManagementSessionToken()).toBe("replacement-token"));
  });

  it("clears the browser session on explicit logout", async () => {
    writeManagementSessionToken("stored-token");
    const api = createApi();
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /log out/i }));

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    expect(readManagementSessionToken()).toBeNull();
    expect(api.logout).toHaveBeenCalledWith("stored-token");
  });

  it("persists a successful bootstrap token and recovers verification through retry", async () => {
    const getBootstrapStatus = vi
      .fn()
      .mockResolvedValueOnce({
        needsBootstrap: true,
        managementConfigured: false,
        environmentOverride: false,
      })
      .mockResolvedValueOnce({
        needsBootstrap: false,
        managementConfigured: true,
        environmentOverride: false,
      });
    const verifySession = vi
      .fn()
      .mockRejectedValueOnce(new Error("verification response was interrupted"))
      .mockResolvedValueOnce(authenticatedSession);
    const api = createApi({ getBootstrapStatus, verifySession });
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <AuthBoundary>
          <div>Console ready</div>
        </AuthBoundary>
      </Providers>,
    );

    expect(
      await screen.findByRole("heading", { name: /初始化管理员/i }),
    ).toBeInTheDocument();
    await user.type(screen.getByLabelText(/^管理密钥$/i), "new-management-token");
    await user.type(
      screen.getByLabelText(/确认管理密钥/i),
      "new-management-token",
    );
    await user.click(screen.getByRole("button", { name: /创建管理员/i }));

    expect(
      await screen.findByRole("heading", { name: /会话不可用/i }),
    ).toBeInTheDocument();
    expect(readManagementSessionToken()).toBe("new-management-token");
    expect(
      screen.queryByRole("heading", { name: /初始化管理员/i }),
    ).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /重新初始化/i }));

    expect(await screen.findByText("Console ready")).toBeInTheDocument();
    expect(getBootstrapStatus).toHaveBeenCalledTimes(2);
    expect(api.bootstrap).toHaveBeenCalledOnce();
    expect(verifySession).toHaveBeenCalledTimes(2);
    expect(verifySession).toHaveBeenLastCalledWith("new-management-token");
  });

  it("recovers when bootstrap commits but its response is lost", async () => {
    const getBootstrapStatus = vi
      .fn()
      .mockResolvedValueOnce({
        needsBootstrap: true,
        managementConfigured: false,
        environmentOverride: false,
      })
      .mockResolvedValueOnce({
        needsBootstrap: false,
        managementConfigured: true,
        environmentOverride: false,
      });
    const api = createApi({
      getBootstrapStatus,
      bootstrap: vi.fn().mockRejectedValue(new Error("bootstrap response was interrupted")),
    });
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <AuthBoundary>
          <div>Console ready</div>
        </AuthBoundary>
      </Providers>,
    );

    expect(
      await screen.findByRole("heading", { name: /初始化管理员/i }),
    ).toBeInTheDocument();
    await user.type(screen.getByLabelText(/^管理密钥$/i), "new-management-token");
    await user.type(
      screen.getByLabelText(/确认管理密钥/i),
      "new-management-token",
    );
    await user.click(screen.getByRole("button", { name: /创建管理员/i }));

    expect(await screen.findByText("Console ready")).toBeInTheDocument();
    expect(getBootstrapStatus).toHaveBeenCalledTimes(2);
    expect(api.verifySession).toHaveBeenCalledWith("new-management-token");
    expect(readManagementSessionToken()).toBe("new-management-token");
  });

  it("expires the in-memory secret grant and resets session access", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-07-22T10:00:00.000Z"));
    try {
      writeManagementSessionToken("stored-token");
      const api = createApi({
        confirmSecretAccess: vi.fn().mockResolvedValue({
          grant: "short-lived-grant",
          expiresAt: "2026-07-22T10:00:01.000Z",
        }),
      });

      render(
        <Providers api={api}>
          <SessionHarness />
        </Providers>,
      );
      await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

      fireEvent.click(screen.getByRole("button", { name: /confirm secret/i }));
      await act(async () => Promise.resolve());
      expect(screen.getByTestId("grant")).toHaveTextContent("short-lived-grant");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("true");

      act(() => vi.advanceTimersByTime(1_001));

      expect(screen.getByTestId("grant")).toHaveTextContent("none");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
    } finally {
      vi.useRealTimers();
    }
  });

  it("reschedules secret grant expiry beyond the browser timer limit", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const start = new Date("2026-07-22T10:00:00.000Z");
    vi.setSystemTime(start);
    try {
      writeManagementSessionToken("stored-token");
      const expiresAt = new Date(start.getTime() + 2_147_483_647 + 1_000).toISOString();
      const api = createApi({
        confirmSecretAccess: vi.fn().mockResolvedValue({
          grant: "long-lived-grant",
          expiresAt,
        }),
      });

      render(
        <Providers api={api}>
          <SessionHarness />
        </Providers>,
      );
      await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

      fireEvent.click(screen.getByRole("button", { name: /confirm secret/i }));
      await act(async () => Promise.resolve());
      expect(screen.getByTestId("grant")).toHaveTextContent("long-lived-grant");

      act(() => vi.advanceTimersByTime(2_147_483_647));
      expect(screen.getByTestId("grant")).toHaveTextContent("long-lived-grant");

      act(() => vi.advanceTimersByTime(1_001));
      expect(screen.getByTestId("grant")).toHaveTextContent("none");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
    } finally {
      vi.useRealTimers();
    }
  });
});
