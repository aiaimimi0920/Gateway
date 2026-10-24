import { cleanup, renderHook, act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { pilotSection, renderWorkspace as renderProviders, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";
import { populatedWorkspaceProps, renderWorkspace as renderGroups } from "./CredentialGroupsWorkspace.fixtures";
import { usePoolPagination } from "./usePoolPagination";
import type { ProviderPoolView } from "./providerPoolView";

afterEach(cleanup);

it("bounds provider cards and defers settings until the first flip", async () => {
  const user = userEvent.setup();
  const settings = vi.fn(() => <span>Lazy settings</span>);
  const sections = Array.from({ length: 49 }, (_, i) => pilotSection({
    providerId: `site-${i}`, providerIds: [`site-${i}`], providerLabel: `Site ${i}`,
  }));
  const { container } = renderProviders(workspaceProps({
    pilotSections: sections, renderProviderSettings: settings, t: (_zh, en) => en,
  }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(24);
  expect(settings).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Flip Site 0 for details" }));
  const settingsNode = screen.getByText("Lazy settings");
  expect(settingsNode).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Flip Site 0 back to the front" }));
  expect(screen.getByText("Lazy settings")).toBe(settingsNode);
  const pager = within(screen.getByRole("navigation", { name: "Credential pools pagination" }));
  await user.click(pager.getByRole("button", { name: "Next page" }));
  expect(container.querySelector("[data-provider-card]")).toHaveAttribute("data-provider-card", "site-24");
  await user.click(pager.getByRole("button", { name: "Next page" }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(1);
});

it("bounds entitlement cards and mounts only the opened scope board", async () => {
  const user = userEvent.setup();
  const base = populatedWorkspaceProps();
  const groups = Array.from({ length: 49 }, (_, i) => ({
    ...base.groups[0], rowId: `row-${i}`, groupId: `group-${i}`, name: `Group ${i}`,
  }));
  const { container } = renderGroups({ ...base, groups });
  expect(container.querySelectorAll("[data-entitlement-group-card]")).toHaveLength(24);
  expect(screen.queryByText("Alt Anthropic")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Flip Group 0 for details" }));
  const scope = screen.getByText("Alt Anthropic");
  expect(scope).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Flip Group 0 back to the front" }));
  expect(screen.getByText("Alt Anthropic")).toBe(scope);
  const pager = within(screen.getByRole("navigation", { name: "Entitlement pools pagination" }));
  await user.click(pager.getByRole("button", { name: "Next page" }));
  expect(container.querySelector("[data-entitlement-group-card]")).toHaveAttribute("data-entitlement-group-card", "group-24");
});

it("resets for search and sort, clamps after deletion, and never mutates results", () => {
  const items = Array.from({ length: 49 }, (_, i) => i);
  const view: ProviderPoolView = { query: "", sort: "default" };
  const { result, rerender } = renderHook(({ items, view }) => usePoolPagination(items, view), {
    initialProps: { items, view },
  });
  act(() => result.current.onPage(3));
  expect(result.current.items).toEqual([48]);
  rerender({ items: items.slice(0, 25), view });
  expect(result.current.page).toBe(2);
  rerender({ items, view: { ...view, query: "new search" } });
  expect(result.current.page).toBe(1);
  act(() => result.current.onPage(2));
  rerender({ items, view: { query: "new search", sort: "name" } });
  expect(result.current.page).toBe(1);
  rerender({ items: [], view });
  expect(result.current.items).toEqual([]);
  expect(result.current.page).toBe(1);
  expect(items).toHaveLength(49);
});
