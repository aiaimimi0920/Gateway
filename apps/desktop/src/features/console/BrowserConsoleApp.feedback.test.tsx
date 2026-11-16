import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { deferred, renderWithProviders, waitForConsoleReady } from "./BrowserConsoleApp.render-fixture";

describe("BrowserConsoleApp standard feedback", () => {
  beforeEach(() => window.localStorage.clear());

  it("preserves all independent warning failures without changing known data or issuing writes", async () => {
    const api = createConsoleApi();
    api.getProviderCredentialInventory = vi.fn().mockRejectedValue(new Error("inventory unavailable"));
    api.getRuntimePressure = vi.fn().mockRejectedValue(new Error("pressure unavailable"));
    const { container } = renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
    await waitForConsoleReady();
    const failures = ["inventory unavailable", "automation unavailable", "refill unavailable", "pressure unavailable"];
    for (const text of failures) {
      const notice = screen.getByText(new RegExp(text)).closest('[role="status"]');
      expect(notice).toHaveClass("n-notice", "n-tone-warning");
    }
    expect(screen.getByRole("status", { name: "Account summary status" })).toHaveTextContent("summary unavailable");
    expect(container.querySelector(".nt-alert, .nt-validation-list")).toBeNull();
    expect(api.commitRouteConfig).not.toHaveBeenCalled();
    expect(api.validateRouteConfig).not.toHaveBeenCalled();
  });

  it("preserves read-only restrictions and their necessary warning", async () => {
    const api = createConsoleApi();
    const response = await api.getRouteConfig("management-secret");
    vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...response.routeConfig, mutationSupported: false } });
    renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
    await waitForConsoleReady();
    const notice = screen.getByText("当前实例是只读模式").closest('[role="status"]');
    expect(notice).toHaveClass("n-notice", "n-tone-warning");
    expect(notice).toHaveTextContent("route-config runtime 没有启用写入支持，浏览器控制台暂时只能查看。");
    expect(screen.getByRole("button", { name: "添加服务商" })).toBeDisabled();
    expect(api.commitRouteConfig).not.toHaveBeenCalled();
  });

  it("uses the standard empty state without redundant usage instructions", async () => {
    const api = createConsoleApi();
    const response = await api.getRouteConfig("management-secret");
    vi.mocked(api.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...response.routeConfig,
        document: { providers: [], model_routes: [], aliases: {}, account_groups: [] }
      }
    });
    renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
    await waitForConsoleReady();
    expect(screen.getByText("暂无账号").closest(".n-empty")).not.toBeNull();
    expect(screen.queryByText(/请使用上方的“添加账号”/)).toBeNull();
    expect(api.commitRouteConfig).not.toHaveBeenCalled();
  });

  it("keeps diagnostics and focus during a failed refresh and clears them after explicit successful retry", async () => {
    const api = createConsoleApi();
    const clean = await api.getRouteConfig("management-secret");
    vi.mocked(api.getRouteConfig).mockResolvedValue({
      routeConfig: {
        ...clean.routeConfig, requiresRepair: true,
        diagnostics: { diagnostics: [{ code: "identity_invalid", severity: "error", path: "/providers/0/id", message: "identity needs repair" }] }
      }
    });
    const user = userEvent.setup();
    renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
    await waitForConsoleReady();
    const diagnostics = screen.getByRole("alert", { name: "Active route diagnostics" });
    const refresh = screen.getByRole("button", { name: "刷新" });
    const pending = deferred<Awaited<ReturnType<typeof api.getRouteConfig>>>();
    vi.mocked(api.getRouteConfig).mockReturnValueOnce(pending.promise);
    await user.click(refresh);
    expect(refresh).toBeDisabled();
    await act(async () => pending.reject(new Error("route retry failed")));
    const error = (await screen.findByText("route retry failed")).closest('[role="alert"]');
    expect(error).toHaveClass("n-notice", "n-tone-danger", "nt-shell-feedback");
    expect(screen.getByRole("alert", { name: "Active route diagnostics" })).toBe(diagnostics);
    expect(within(diagnostics).getByText("identity needs repair", { exact: false })).toBeInTheDocument();
    await waitFor(() => expect(refresh).toBeEnabled());
    expect(refresh).toHaveFocus();
    vi.mocked(api.getRouteConfig).mockResolvedValue(clean);
    await user.click(refresh);
    await waitFor(() => expect(screen.queryByRole("alert", { name: "Active route diagnostics" })).toBeNull());
    expect(screen.queryByText("route retry failed")).toBeNull();
    expect(api.commitRouteConfig).not.toHaveBeenCalled();
  });
});
