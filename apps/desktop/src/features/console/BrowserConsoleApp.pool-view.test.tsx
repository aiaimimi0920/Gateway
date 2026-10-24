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
    screen.queryByRole("heading", { name: "凭据池" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "站点健康" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: /添加服务商/ }),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: /添加账号/ })).toBeInTheDocument();
  const search = screen.getByRole("searchbox", { name: "搜索站点" });
  await user.type(search, "BETA.EXAMPLE");
  expect(screen.getByRole("button", { name: "Beta" })).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Alpha" }),
  ).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "清除站点搜索" }));
  await user.click(screen.getByRole("combobox", { name: "站点排序" }));
  await user.click(screen.getByRole("option", { name: "名称升序" }));
  expect(
    [...document.querySelectorAll("[data-provider-card-stack]")].map((card) =>
      card.getAttribute("data-provider-card-stack"),
    ),
  ).toEqual(["alpha", "beta"]);
  await user.type(search, "missing.example");
  expect(
    screen.getByRole("heading", { name: "没有匹配站点" }),
  ).toBeInTheDocument();
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.clear(search);
  await user.click(screen.getByRole("button", { name: "翻面查看 Alpha 详情" }));
  const card = document.querySelector(
    '[data-provider-card="alpha"]',
  ) as HTMLElement;
  await user.click(within(card).getByRole("button", { name: "站点健康" }));
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
