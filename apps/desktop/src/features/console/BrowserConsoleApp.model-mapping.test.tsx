import { screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi, preserveCommittedRouteDocument } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace, waitForCommittedRouteDraft, providerCard } from "./BrowserConsoleApp.render-fixture";

it("autosaves mapping changes while staying open, and reloads them after closing", async () => {
  window.localStorage.clear();
  const api = createConsoleApi();
  const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...initial.routeConfig, document: {
    providers: [{ id: "pool", preset: "openai", label: "Mapping Pool", supported_models: ["b", "c"],
      model_map: { a: "b" }, credentials: [{ id: "one", account_name: "One" }] }],
    model_routes: [], account_groups: [], aliases: {},
  }, mutationSupported: true } });
  await preserveCommittedRouteDocument(api);
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />, {
    session: { secretAccessGranted: true }, secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
  });
  await waitForConsoleReady(); await openWorkspace(user, /凭据池/i);
  const open = () => user.click(within(providerCard(/^Mapping Pool$/)).getByRole("button", { name: "模型映射" }));
  await open();
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.click(screen.getByRole("checkbox", { name: "c" }));
  const draft = await waitForCommittedRouteDraft(api);
  expect(draft.providers[0]).toMatchObject({ model_map_targets: { a: ["b", "c"] }, supported_models: ["b", "c"] });
  expect(screen.getByRole("dialog", { name: "模型映射 · Mapping Pool" })).toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole("button", { name: "删除映射模型" })).toBeEnabled());
  await user.click(screen.getByRole("button", { name: "关闭" }));
  await open();
  expect(screen.getByRole("checkbox", { name: "b" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "c" })).toBeChecked();
  const count = vi.mocked(api.commitRouteConfig).mock.calls.length;
  await user.click(screen.getByRole("button", { name: "删除映射模型" }));
  const cleared = await waitForCommittedRouteDraft(api, count);
  expect(cleared.providers[0]).not.toHaveProperty("model_map");
  expect(cleared.providers[0]).not.toHaveProperty("model_map_targets");
  expect(cleared.providers[0]).toMatchObject({ supported_models: ["b", "c"], credentials: [{ id: "one" }] });
  expect(api.probeProvider).not.toHaveBeenCalled();
  expect(api.probeCredential).not.toHaveBeenCalled();
});
it("keeps the mapping dialog and draft on persistence failure, then saves the next edit", async () => {
  window.localStorage.clear();
  const api = createConsoleApi();
  const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...initial.routeConfig, document: {
    providers: [{ id: "pool", preset: "openai", label: "Mapping Pool", supported_models: ["b", "c"],
      model_map: { a: "b" }, credentials: [{ id: "one", account_name: "One" }] }],
    model_routes: [], account_groups: [], aliases: {},
  }, mutationSupported: true } });
  await preserveCommittedRouteDocument(api);
  vi.mocked(api.commitRouteConfig).mockRejectedValueOnce(new Error("Mapping persistence failed"));
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />, {
    session: { secretAccessGranted: true }, secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
  });
  await waitForConsoleReady(); await openWorkspace(user, /凭据池/i);
  await user.click(within(providerCard(/^Mapping Pool$/)).getByRole("button", { name: "模型映射" }));
  await user.click(screen.getByRole("checkbox", { name: "c" }));
  const dialog = screen.getByRole("dialog", { name: "模型映射 · Mapping Pool" });
  await waitFor(() => expect(within(dialog).getByRole("alert")).toHaveTextContent("Mapping persistence failed"), { timeout: 3000 });
  expect(screen.getByRole("checkbox", { name: "c" })).toBeChecked();
  expect(api.commitRouteConfig).toHaveBeenCalledTimes(1);
  await user.click(screen.getByRole("checkbox", { name: "b" }));
  const retry = await waitForCommittedRouteDraft(api, 1);
  expect(retry.providers[0]).toMatchObject({ model_map: { a: "c" } });
  await waitFor(() => expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument());
}, 10000);