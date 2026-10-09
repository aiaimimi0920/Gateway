import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace, waitForCommittedRouteDraft, providerCard } from "./BrowserConsoleApp.render-fixture";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";

it("opens the pool plan list and autosaves a named plan without rewriting an account policy or calling a model", async () => {
  window.localStorage.clear();
  const api = createConsoleApi();
  const initial = await api.getRouteConfig("management-secret");
  vi.mocked(api.getRouteConfig).mockResolvedValue({ routeConfig: { ...initial.routeConfig, document: {
    providers: [{ id: "pool", preset: "openai", label: "Plan Pool", supported_models: ["a"],
      credentials: [{ id: "one", account_name: "One", test_policy: defaultTestPolicy() }] }],
    model_routes: [], account_groups: [], aliases: {},
  }, mutationSupported: true } });
  const user = userEvent.setup();
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />, {
    session: { secretAccessGranted: true }, secretGrant: { grant: "grant-1", expiresAt: "2099-01-01T00:00:00Z" },
  });
  await waitForConsoleReady(); await openWorkspace(user, /凭据池/i);
  await user.click(within(providerCard(/^Plan Pool$/)).getByRole("button", { name: "测试" }));
  expect(screen.getByRole("tab", { name: "测试" })).toBeInTheDocument();
  expect(screen.queryByRole("textbox", { name: "计划名称" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  await user.type(screen.getByRole("textbox", { name: "计划名称" }), "连接计划");
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  const draft = await waitForCommittedRouteDraft(api);
  expect(draft.providers[0]).toMatchObject({ test_plans: [expect.objectContaining({ name: "连接计划", scopes: [{ kind: "pool" }],
    policy: expect.objectContaining({ automaticEnabled: false }) })], credentials: [{ id: "one", test_policy: defaultTestPolicy() }] });
  expect(api.probeProvider).not.toHaveBeenCalled();
  expect(api.probeCredential).not.toHaveBeenCalled();
});
