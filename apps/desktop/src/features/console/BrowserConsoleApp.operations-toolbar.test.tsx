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
  expect(await screen.findByText("已加载记录：2 / 2")).toBeVisible();
  expect(screen.getAllByRole("heading", { name: /^运维$/, level: 1 })).toHaveLength(1);
  const requestsTable = within(screen.getByRole("table", { name: "请求列表" }));
  expect(requestsTable.getAllByRole("button", { name: /请求详情:/ }).map(button => button.getAttribute("aria-label"))).toEqual(["请求详情: alpha", "请求详情: beta"]);
  const search = screen.getByRole("textbox", { name: "请求 ID" });
  const initialCalls = vi.mocked(api.listRequestAudits).mock.calls.length;
  await user.type(search, "ALPHA");
  expect(screen.getByText("Alpha")).toBeVisible();
  expect(screen.queryByText("Beta")).not.toBeInTheDocument();
  expect(screen.getByText("已加载记录：1 / 2")).toBeVisible();
  await user.clear(search);
  await user.type(search, "nothing-matches");
  expect(screen.getByRole("status")).toHaveTextContent("没有匹配的请求");
  await user.click(screen.getByRole("button", { name: "重置" }));
  expect(screen.getByText("已加载记录：2 / 2")).toBeVisible();
  expect(search).toHaveValue("");
  expect(api.listRequestAudits).toHaveBeenCalledTimes(initialCalls);
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
}, 15000);
