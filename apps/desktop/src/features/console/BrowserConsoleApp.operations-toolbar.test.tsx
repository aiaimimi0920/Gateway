import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { openWorkspace, renderWithProviders, waitForConsoleReady } from "./BrowserConsoleApp.render-fixture";
import { operationsRequestFixture, operationsSummaryFixture } from "./OperationsWorkspace.fixtures";

beforeEach(() => window.localStorage.clear());

it("places functional operations search/sort in the shell without submitting server filters or changing totals", async () => {
  const api = createConsoleApi();
  const requests = [
    { ...operationsRequestFixture("beta"), requestedModel: "Beta", createdAt: "2026-10-24T01:00:00Z" },
    { ...operationsRequestFixture("alpha"), requestedModel: "Alpha", createdAt: "2026-10-24T02:00:00Z" },
  ];
  api.listRequestAudits = vi.fn().mockResolvedValue({ requests });
  api.getOperatorSummary = vi.fn().mockResolvedValue({ summary: operationsSummaryFixture(false) });
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  await waitForConsoleReady();
  await openWorkspace(user, /^运维$/);
  expect(await screen.findByText("本次查询 2 条，失败 2 条")).toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: /^运维$/ })).not.toBeInTheDocument();
  expect(screen.queryByText(/先看运行状态|基础运维|概况采集于/)).not.toBeInTheDocument();
  const search = screen.getByRole("searchbox", { name: "搜索运维记录" });
  expect(search.closest(".nt-board__header")).not.toBeNull();
  expect(search).toHaveAttribute("maxLength", "256");
  const initialCalls = vi.mocked(api.listRequestAudits).mock.calls.length;
  await user.type(search, "ALPHA");
  expect(screen.getByText("Alpha")).toBeVisible();
  expect(screen.queryByText("Beta")).not.toBeInTheDocument();
  expect(screen.getByText("本次查询 2 条，失败 2 条，匹配 1 条")).toBeVisible();
  await user.click(screen.getByRole("tab", { name: "请求排查" }));
  expect(screen.getByText("Alpha")).toBeVisible();
  expect(screen.queryByText("Beta")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "清除运维搜索" }));
  const models = () => [...document.querySelectorAll(".nt-table--requests .nt-operations-request-model > span")].map((node) => node.textContent);
  expect(models()).toEqual(["Beta", "Alpha"]);
  await user.click(screen.getByRole("combobox", { name: "运维排序" }));
  await user.click(screen.getByRole("option", { name: "时间降序" }));
  expect(models()).toEqual(["Alpha", "Beta"]);
  await user.click(screen.getByRole("combobox", { name: "运维排序" }));
  await user.click(screen.getByRole("option", { name: "名称降序" }));
  expect(models()).toEqual(["Beta", "Alpha"]);
  await user.type(search, "nothing-matches");
  expect(screen.getByText("没有匹配的请求")).toBeVisible();
  expect(api.listRequestAudits).toHaveBeenCalledTimes(initialCalls);
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await openWorkspace(user, /^密钥$/);
  await openWorkspace(user, /^运维$/);
  expect(screen.getByRole("searchbox", { name: "搜索运维记录" })).toHaveValue("nothing-matches");
  expect(screen.getByRole("combobox", { name: "运维排序" })).toHaveTextContent("名称降序");
  expect(within(screen.getByRole("tablist", { name: "运维视图" })).getByRole("tab", { name: "运行概览" })).toHaveAttribute("aria-selected", "true");
}, 15000);
