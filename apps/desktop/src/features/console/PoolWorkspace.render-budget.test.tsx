import { cleanup, renderHook, act, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";
import { pilotSection, renderWorkspace as renderProviders, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";
import { populatedWorkspaceProps, renderWorkspace as renderGroups } from "./CredentialGroupsWorkspace.fixtures";
import { usePoolPagination } from "./usePoolPagination";
import type { ProviderPoolView } from "./providerPoolView";
import { buildAccountLedgerRows } from "./accountManagementViewModel";
import { buildRouteAccountCatalog } from "./routeDocumentAccountCatalog";

afterEach(cleanup);

it("appends provider batches without remounting existing cards or their lazy settings", async () => {
  const user = userEvent.setup();
  const settings = vi.fn(() => <span>Lazy settings</span>);
  const sections = Array.from({ length: 49 }, (_, i) => pilotSection({
    providerId: `site-${i}`, providerIds: [`site-${i}`], providerLabel: `Site ${i}`,
  }));
  const { container } = renderProviders(workspaceProps({
    pilotSections: sections, renderProviderSettings: settings, t: (_zh, en) => en,
  }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(24);
  const firstCard = container.querySelector('[data-provider-card="site-0"]');
  expect(settings).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Flip Site 0 for details" }));
  const settingsNode = screen.getByText("Lazy settings");
  expect(settingsNode).toBeVisible();
  await user.click(screen.getByRole("button", { name: "Flip Site 0 back to the front" }));
  expect(screen.getByText("Lazy settings")).toBe(settingsNode);
  expect(screen.queryByRole("navigation", { name: "Credential pools pagination" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Load more" }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(48);
  expect(container.querySelector('[data-provider-card="site-0"]')).toBe(firstCard);
  expect(screen.getByText("Lazy settings")).toBe(settingsNode);
  await user.click(screen.getByRole("button", { name: "Load more" }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(49);
  expect(screen.queryByRole("button", { name: "Load more" })).not.toBeInTheDocument();
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

it("reveals legacy rows only after provider cards and appends them in the same feed", async () => {
  const user = userEvent.setup();
  const sections = Array.from({ length: 25 }, (_, index) => pilotSection({
    providerId: `site-${index}`, providerIds: [`site-${index}`], providerLabel: `Site ${index}`,
  }));
  const rows = buildAccountLedgerRows(buildRouteAccountCatalog({
    providers: Array.from({ length: 30 }, (_, index) => ({
      id: `legacy-${index}`, label: `Legacy ${index}`, base_url: "https://example.invalid", api_key: "fixture",
    })), model_routes: [], aliases: {},
  }), {});
  const { container } = renderProviders(workspaceProps({
    pilotSections: sections, rows, t: (_zh, en) => en,
  }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(24);
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Load more" }));
  expect(container.querySelectorAll("[data-provider-card]")).toHaveLength(25);
  const table = screen.getByRole("table", { name: "Account ledger table" });
  expect(within(table).getAllByRole("row")).toHaveLength(24);
  const first = within(table).getAllByRole("row")[1];
  await user.click(screen.getByRole("button", { name: "Load more" }));
  expect(within(table).getAllByRole("row")).toHaveLength(31);
  expect(within(table).getAllByRole("row")[1]).toBe(first);
  expect(screen.queryByRole("button", { name: "Load more" })).not.toBeInTheDocument();
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
