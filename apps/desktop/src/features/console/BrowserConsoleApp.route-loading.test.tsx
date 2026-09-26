import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { deferred, renderWithProviders } from "./BrowserConsoleApp.render-fixture";

describe("route-independent console navigation", () => {
  beforeEach(() => window.localStorage.clear());

  it.each([
    { navigation: /Gateway console navigation/i, label: /^运维$/, content: /^实时$/ },
    { navigation: /Gateway console navigation/i, label: /^访问密钥$/, content: /^生效密钥$/ },
    { navigation: /辅助导航/i, label: /^设置$/, content: /^界面语言$/ },
  ])("keeps $label reachable while the initial route request is pending or fails", async ({ navigation, label, content }) => {
    const api = createConsoleApi();
    const request = deferred<Awaited<ReturnType<typeof api.getRouteConfig>>>();
    vi.mocked(api.getRouteConfig).mockReturnValue(request.promise);
    const user = userEvent.setup();
    renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
    expect(await screen.findByText("正在加载 Gateway 控制台...")).toBeInTheDocument();
    const nav = screen.getByRole("navigation", { name: navigation });
    await user.click(within(nav).getByRole("button", { name: label }));
    expect(await screen.findByText(content)).toBeInTheDocument();
    await act(async () => request.reject(new Error("route storage unavailable")));
    expect(screen.getByText(content)).toBeInTheDocument();
    expect(screen.getByText("route storage unavailable")).toBeInTheDocument();
    expect(api.commitRouteConfig).not.toHaveBeenCalled();
  });
});
