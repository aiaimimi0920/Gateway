import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AuthBoundary } from "../features/auth/AuthBoundary";
import { readManagementSessionToken, writeManagementSessionToken } from "./storage";
import { authenticatedSession, createApi, Providers, SessionHarness } from "./managementSessionTestFixtures";

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

});
