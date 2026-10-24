import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { openWorkspace, renderWithProviders, waitForConsoleReady } from "./BrowserConsoleApp.render-fixture";

beforeEach(() => window.localStorage.clear());

it("replaces group/model headings with independent search/sort controls without saving routes", async () => {
  const api = createConsoleApi();
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  await waitForConsoleReady();
  await openWorkspace(user, /权益组/i);
  expect(screen.queryByRole("heading", { name: "权益组" })).not.toBeInTheDocument();
  await user.type(screen.getByRole("searchbox", { name: "搜索权益池" }), "premium");
  await user.click(screen.getByRole("combobox", { name: "权益池排序" }));
  await user.click(screen.getByRole("option", { name: "调用次数最多" }));
  await openWorkspace(user, /模型池/i);
  expect(screen.queryByRole("heading", { name: "模型池" })).not.toBeInTheDocument();
  expect(screen.getByRole("searchbox", { name: "搜索模型" })).toHaveValue("");
  await user.type(screen.getByRole("searchbox", { name: "搜索模型" }), "gemini");
  await user.click(screen.getByRole("combobox", { name: "模型排序" }));
  await user.click(screen.getByRole("option", { name: "名称升序" }));
  await openWorkspace(user, /权益组/i);
  expect(screen.getByRole("searchbox", { name: "搜索权益池" })).toHaveValue("premium");
  expect(screen.getByRole("combobox", { name: "权益池排序" })).toHaveTextContent("调用次数最多");
  await user.click(screen.getByRole("button", { name: "清除权益池搜索" }));
  expect(screen.getByRole("searchbox", { name: "搜索权益池" })).toHaveValue("");
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
});
