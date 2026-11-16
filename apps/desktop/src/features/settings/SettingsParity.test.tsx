import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it } from "vitest";
import { renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ConsoleSettingsWorkspace } from "./ConsoleSettingsWorkspace";

afterEach(() => { delete window.__GATEWAY_WINDOW_CHROME__; });

it("shows only shared categories in browsers and navigates only visible tabs", async () => {
  renderWithProviders(<ConsoleSettingsWorkspace />);
  const tabs = within(screen.getByRole("tablist"));
  expect(tabs.getAllByRole("tab").map(tab => tab.textContent)).toEqual(["常规", "网络与共享", "可观测性", "同步与备份", "关于"]);
  const user = userEvent.setup();
  tabs.getByRole("tab", { name: "常规" }).focus();
  await user.keyboard("{End}");
  expect(tabs.getByRole("tab", { name: "关于" })).toHaveFocus();
  await user.keyboard("{ArrowRight}");
  expect(tabs.getByRole("tab", { name: "常规" })).toHaveFocus();
  await user.keyboard("{ArrowLeft}");
  expect(tabs.getByRole("tab", { name: "关于" })).toHaveFocus();
  await user.keyboard("{Home}");
  expect(tabs.getByRole("tab", { name: "常规" })).toHaveFocus();
});
