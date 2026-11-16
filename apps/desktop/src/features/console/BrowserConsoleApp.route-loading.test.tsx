import { act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { deferred, renderWithProviders } from "./BrowserConsoleApp.render-fixture";

describe("route-independent console navigation", () => {
  beforeEach(() => window.localStorage.clear());

  it.each([
    { navigation: /Gateway console navigation/i, label: /^运维$/, content: /^请求列表$/ },
    { navigation: /Gateway console navigation/i, label: /^密钥$/, content: /^新建密钥$/ },
    { navigation: /辅助导航/i, label: /^设置$/, content: /^语言$/, role: "combobox" as const },
  ])("keeps $label reachable while the initial route request is pending or fails", async ({ navigation, label, content, role }) => {
    const api = createConsoleApi();
    const request = deferred<Awaited<ReturnType<typeof api.getRouteConfig>>>();
    vi.mocked(api.getRouteConfig).mockReturnValue(request.promise);
    const user = userEvent.setup();
    const findContent = () => role ? screen.findByRole(role, { name: content }) : screen.findByText(content);
    renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
    expect((await screen.findByText("正在加载 Gateway 控制台...")).closest('[role="status"]')).toHaveClass("n-notice");
    const nav = screen.getByRole("navigation", { name: navigation });
    await user.click(within(nav).getByRole("button", { name: label }));
    expect(await findContent()).toBeInTheDocument();
    if (label.test("运维")) {
      expect(screen.getByRole("heading", { name: /^运维$/, level: 1 })).toBeVisible();
      expect(screen.getByRole("textbox", { name: "请求 ID" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "重置" })).toBeInTheDocument();
    }
    if (label.test("密钥")) {
      expect(screen.getByRole("heading", { name: /^密钥$/, level: 1 })).toBeVisible();
      expect(screen.getByRole("searchbox", { name: "搜索密钥" })).toBeInTheDocument();
      expect(screen.getByRole("combobox", { name: "密钥排序" })).toBeInTheDocument();
      expect(screen.queryByText(/运营侧签发与吊销/)).not.toBeInTheDocument();
      expect(document.querySelector(".nt-console-toolbar")).not.toBeInTheDocument();
    }
    await act(async () => request.reject(new Error("route storage unavailable")));
    expect(await findContent()).toBeInTheDocument();
    expect(screen.getByText("route storage unavailable")).toBeInTheDocument();
    expect(api.commitRouteConfig).not.toHaveBeenCalled();
  });
});
