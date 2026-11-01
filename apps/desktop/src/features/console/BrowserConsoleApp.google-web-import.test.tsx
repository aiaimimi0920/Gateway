import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import type { ConsoleGeminiAuthFamily } from "../../api/contracts";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady } from "./BrowserConsoleApp.render-fixture";

beforeEach(() => window.localStorage.clear());
function setup(preset: string, family: ConsoleGeminiAuthFamily, succeeded = false) {
  const api = createConsoleApi();
  const baseUrl = family === "stitch-web" ? "https://stitch.withgoogle.com" : family === "flow-web" ? "https://flow.google.com" : family === "aistudio-web" ? "https://aistudio.google.com" : "https://gemini.google.com";
  const existing = { id: "existing", runtime_state_object_key: "keep-existing-object", supported_models: ["existing-model"] };
  let config = { revision: { id: "r1-import", sequence: 1 }, source: "redis" as const,
    diagnostics: { diagnostics: [] }, requiresRepair: false, mutationSupported: true, secrets: [],
    document: { providers: [{ id: "custom-pool", preset, base_url: baseUrl, credentials: [existing] }],
      model_routes: [], aliases: {}, account_groups: [] } };
  vi.mocked(api.getRouteConfig).mockImplementation(async () => ({ routeConfig: config }));
  vi.mocked(api.commitRouteConfig).mockImplementation(async (_token, request) => {
    config = { ...config, revision: { id: "r2-import", sequence: 2 }, document: request.document as typeof config.document };
    return { committed: true, routeConfig: config };
  });
  const session = { id: "selected-session", targetFamily: family, providerId: "custom-pool",
    status: "waiting_user" as const, message: "Complete normal sign-in.", createdAt: "2026-11-01T00:00:00Z",
    updatedAt: "2026-11-01T00:00:00Z", generatedDrafts: [] };
  vi.mocked(api.createGeminiAuthSession).mockResolvedValue({ session });
  vi.mocked(api.getGeminiAuthSession).mockResolvedValue({ session: succeeded ? { ...session, status: "succeeded",
    generatedDrafts: [{ providerId: "custom-pool", credential: { id: "new-ai-account", account_name: "My AI Studio",
      runtime_state_object_key: "credential-runtime/aistudio/fixture/http-cookies.json",
      supported_models: ["gemini-3.5-flash-lite"], model_selection: { fetched: ["gemini-3.5-flash-lite"], manual: [] } }, secretEdits: [] }] } : session });
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  return api;
}

it.each([
  ["aistudio-web-reverse", "aistudio-web", "AI Studio"],
  ["gemini-web-chat", "gemini-web", "Gemini Web"],
  ["gemini-canvas-browser-relay", "gemini-canvas", "Gemini Canvas"],
  ["flow-web", "flow-web", "Google Flow"],
  ["stitch-web", "stitch-web", "Google Stitch"],
] as const)("shows the dedicated %s import in the actual add-account dialog", async (preset, family, name) => {
  const api = setup(preset, family); const user = userEvent.setup();
  await waitForConsoleReady();
  await user.click(screen.getByRole("button", { name: /^添加账号$/ }));
  const dialog = screen.getByRole("dialog", { name: "新增账号" });
  await user.selectOptions(within(dialog).getByLabelText("Provider"), "custom-pool");
  expect(within(dialog).queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.type(within(dialog).getByLabelText("账号名称"), "My account");
  await user.click(within(dialog).getByRole("button", { name: `${name} 网页登录并导入` }));
  await waitFor(() => expect(api.createGeminiAuthSession).toHaveBeenCalledWith("management-secret", {
    providerId: "custom-pool", targetFamily: family, accountLabel: "My account",
  }));
  expect(screen.queryByRole("dialog", { name: "新增账号" })).not.toBeInTheDocument();
  expect(api.probeProvider).not.toHaveBeenCalled();
});

it("persists an AI Studio session into its selected pool and synchronizes models without losing an existing account", async () => {
  const api = setup("aistudio-web-reverse", "aistudio-web", true); const user = userEvent.setup();
  await waitForConsoleReady();
  await user.click(screen.getByRole("button", { name: /^添加账号$/ }));
  await user.selectOptions(screen.getByLabelText("Provider"), "custom-pool");
  await user.click(screen.getByRole("button", { name: "AI Studio 网页登录并导入" }));
  await waitFor(() => expect(api.commitRouteConfig).toHaveBeenCalled());
  const document = vi.mocked(api.commitRouteConfig).mock.calls[0][1].document;
  const provider = document.providers[0] as Record<string, unknown>;
  expect(provider.credentials).toEqual(expect.arrayContaining([
    expect.objectContaining({ id: "existing", runtime_state_object_key: "keep-existing-object" }),
    expect.objectContaining({ id: "new-ai-account", runtime_state_object_key: "credential-runtime/aistudio/fixture/http-cookies.json" }),
  ]));
  expect(document.model_routes).toContainEqual(expect.objectContaining({ pattern: "gemini-3.5-flash-lite", provider_ids: ["custom-pool"] }));
  expect(api.completeGeminiAuthSession).not.toHaveBeenCalled();
});
