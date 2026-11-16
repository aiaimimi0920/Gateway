import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace, waitForCommittedRouteDraft } from "./BrowserConsoleApp.render-fixture";

it("confirms and automatically saves whole-pool removal with secret/reference preservation", async () => {
  window.localStorage.clear();
  const api = createConsoleApi();
  const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...initial.routeConfig, document: {
    providers: [
      { id: "pool", preset: "openai", label: "Delete Pool", supported_models: ["a"], credentials: [{ id: "one" }, { id: "two" }] },
      { id: "other", preset: "openai", label: "Keep Pool", credentials: [{ id: "keep" }] },
    ], model_routes: [{ pattern: "a", provider_ids: ["pool", "other"] }],
    account_groups: [{ id: "group", name: "Group", provider_credential_ids: ["one", "two", "keep"] }], aliases: {},
  }, mutationSupported: true } });
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />, {
    session: { secretAccessGranted: true }, secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
  });
  await waitForConsoleReady();
  await openWorkspace(user, /凭据池/i);
  await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /^Delete Pool/ }));
  await user.click(screen.getByLabelText("更多操作"));
  const remove = screen.getByRole("button", { name: "删除服务商" });
  await user.click(remove);
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "取消" }));
  await waitFor(() => expect(remove).toHaveFocus());
  expect(api.commitRouteConfig).not.toHaveBeenCalled();
  await user.click(remove);
  await user.click(screen.getByRole("button", { name: "确认删除凭据池" }));
  const draft = await waitForCommittedRouteDraft(api);
  expect(draft.providers).toEqual([expect.objectContaining({ id: "other" })]);
  expect(draft.model_routes).toEqual([{ pattern: "a", provider_ids: ["other"] }]);
  expect(draft.account_groups).toEqual([expect.objectContaining({ provider_credential_ids: ["keep"] })]);
});
