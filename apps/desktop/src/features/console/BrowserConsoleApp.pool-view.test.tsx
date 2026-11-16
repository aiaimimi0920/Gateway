import { screen, within } from "@testing-library/react";
import { http, HttpResponse } from "msw";
import { server } from "../../test/server";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import {
  openWorkspace,
  renderWithProviders,
  waitForConsoleReady,
} from "./BrowserConsoleApp.render-fixture";

beforeEach(() => window.localStorage.clear());

it("replaces the right-side pool title with site search/sort while retaining navigation and add actions", async () => {
  server.use(
    http.get("*/v1/internal/gateway/site-health", () =>
      HttpResponse.json({ items: [] }),
    ),
  );
  const api = createConsoleApi();
  const config = await api.getRouteConfig("fixture");
  config.routeConfig.document.providers = [
    {
      id: "beta",
      label: "Beta",
      preset: "openai",
      base_url: "https://beta.example",
      api_key: "fixture",
    },
    {
      id: "alpha",
      label: "Alpha",
      preset: "openai",
      base_url: "https://alpha.example",
      api_key: "fixture",
    },
  ];
  config.routeConfig.document.model_routes = [];
  vi.mocked(api.getRouteConfig).mockResolvedValue(config);
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  await waitForConsoleReady();
  await openWorkspace(user, /凭据池/i);
  expect(
    screen.getByRole("heading", { name: "凭据池", level: 1 }),
  ).toBeVisible();
  expect(
    screen.queryByRole("button", { name: "站点健康" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: /添加服务商/ }),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /添加账号/ })).toBeInTheDocument();
  const search = screen.getByRole("searchbox", { name: "搜索站点" });
  await user.click(screen.getByRole("button", { name: "OAuth 授权" }));
  expect(within(screen.getByRole("navigation", { name: "服务商目录" })).queryByRole("button", { name: /^Beta/ })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "API Key" }));
  await user.type(search, "BETA.EXAMPLE");
  expect(screen.getByRole("button", { name: /^BetaAPI Key/ })).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /^AlphaAPI Key/ }),
  ).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "清除站点搜索" }));
  await user.click(screen.getByRole("combobox", { name: "站点排序" }));
  await user.click(screen.getByRole("option", { name: "名称升序" }));
  expect(
    within(screen.getByRole("navigation", { name: "服务商目录" })).getAllByRole("button").slice(1).map(button => button.textContent),
  ).toEqual(["AlphaAPI Key1", "BetaAPI Key1"]);
  await user.type(search, "missing.example");
  expect(within(screen.getByRole("navigation", { name: "服务商目录" })).getAllByRole("button")).toHaveLength(1);
  expect(screen.getByText("尚未添加账号")).toBeVisible();
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.clear(search);
  await user.click(screen.getByRole("button", { name: /^AlphaAPI Key/ }));
  await user.click(screen.getByRole("button", { name: "服务商设置" }));
  await user.click(within(screen.getByRole("dialog", { name: "服务商设置" })).getByRole("button", { name: "站点健康" }));
  expect(
    screen.getByRole("dialog", { name: "Alpha · 站点健康" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("combobox", { name: "站点" }),
  ).not.toBeInTheDocument();
  const health = within(
    screen.getByRole("dialog", { name: "Alpha · 站点健康" }),
  );
  expect(health.getByRole("tablist", { name: "检测方案" })).toBeInTheDocument();
  expect(
    health.getByRole("tab", { name: "默认方案", selected: true }),
  ).toBeInTheDocument();
  expect(
    health.queryByRole("combobox", { name: "检测方案" }),
  ).not.toBeInTheDocument();
  expect(
    health.getByRole("button", { name: "保存站点策略" }),
  ).toHaveTextContent("保存方案");
  await user.click(health.getByRole("switch", { name: "启用此方案" }));
  await user.selectOptions(health.getByLabelText("适配器"), "openai-status");
  expect(health.getByLabelText("监控站地址（留空使用本站）")).toHaveValue(
    "https://status.openai.com/",
  );
  expect(health.getByLabelText("监控站地址（留空使用本站）")).toHaveAttribute(
    "readonly",
  );
  expect(health.getByText("认证方式：无需认证")).toBeInTheDocument();
}, 15000);
