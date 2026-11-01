import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import type { ConsoleGeminiAuthFamily } from "../../api/contracts";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady } from "./BrowserConsoleApp.render-fixture";

beforeEach(() => window.localStorage.clear());
it.each([
  ["aistudio-web-reverse", "aistudio-web"],
  ["gemini-web-chat", "gemini-web"],
  ["gemini-canvas-browser-relay", "gemini-canvas"],
  ["flow-web", "flow-web"],
  ["stitch-web", "stitch-web"],
] as const)("imports %s JSON through the actual add-account dialog and normal CAS save", async (preset, family: ConsoleGeminiAuthFamily) => {
  const api = createConsoleApi(), user = userEvent.setup();
  let config = { revision: { id: "r1-json", sequence: 1 }, source: "redis" as const,
    diagnostics: { diagnostics: [] }, requiresRepair: false, mutationSupported: true, secrets: [],
    document: { providers: [{ id: "custom-pool", preset, base_url: family === "stitch-web" ? "https://stitch.withgoogle.com" : family === "flow-web" ? "https://flow.google.com" : family === "aistudio-web" ? "https://aistudio.google.com" : "https://gemini.google.com",
      credentials: [{ id: "existing", runtime_state_object_key: "preserve-existing-object" }] }], model_routes: [], aliases: {}, account_groups: [] } };
  vi.mocked(api.getRouteConfig).mockImplementation(async () => ({ routeConfig: config }));
  vi.mocked(api.commitRouteConfig).mockImplementation(async (_token, request) => {
    config = { ...config, revision: { id: "r2-json", sequence: 2 }, document: request.document as typeof config.document };
    return { committed: true, routeConfig: config };
  });
  const secretEdits = family === "gemini-web" ? [{ field: "api_key" as const, operation: "replace" as const, value: "synthetic-private-cookie" }] : [];
  const models = family === "flow-web" ? [] : family === "stitch-web" ? ["stitch-design"] : ["json-model"];
  vi.mocked(api.createGeminiAuthSession).mockResolvedValue({ session: { id: "json-session", providerId: "custom-pool", targetFamily: family,
    status: "succeeded", message: "Offline JSON draft; not verified online", createdAt: "now", updatedAt: "now",
    generatedDrafts: [{ providerId: "custom-pool", credential: { id: "json-account", account_name: "JSON account", supported_models: models,
      ...(family === "gemini-web" ? {} : { runtime_state_object_key: "credential-runtime/json-test/http-cookies.json" }),
      model_selection: { fetched: [], manual: models } }, secretEdits }] } });
  renderWithProviders(<BrowserConsoleApp consoleApi={api} />, { session: { secretAccessGranted: true },
    secretGrant: { grant: "json-grant", expiresAt: "2099-01-01T00:00:00Z" } });
  await waitForConsoleReady(); await user.click(screen.getByRole("button", { name: /^添加账号$/ }));
  const dialog = screen.getByRole("dialog", { name: "新增账号" });
  await user.selectOptions(within(dialog).getByLabelText("Provider"), "custom-pool");
  await user.type(within(dialog).getByLabelText("账号名称"), "JSON account");
  await user.selectOptions(within(dialog).getByLabelText("录入方式"), "json");
  const source = JSON.stringify({ schemaVersion: 1, provider: family, authUser: "0", cookies: [] });
  fireEvent.change(within(dialog).getByLabelText("网页会话 JSON"), { target: { value: source } });
  await user.click(within(dialog).getByRole("button", { name: "导入 JSON" }));
  await waitFor(() => expect(api.createGeminiAuthSession).toHaveBeenCalledWith("management-secret", {
    providerId: "custom-pool", targetFamily: family, accountLabel: "JSON account", webSessionJson: source, expectedRevision: "r1-json",
  }, "json-grant"));
  await waitFor(() => expect(api.commitRouteConfig).toHaveBeenCalled(), { timeout: 3000 });
  const request = vi.mocked(api.commitRouteConfig).mock.calls[0][1];
  expect(request.expectedRevision).toBe("r1-json");
  expect((request.document.providers[0] as Record<string, unknown>).credentials).toEqual(expect.arrayContaining([
    expect.objectContaining({ id: "existing", runtime_state_object_key: "preserve-existing-object" }), expect.objectContaining({ id: "json-account" }),
  ]));
  expect(JSON.stringify(request.document)).not.toContain("synthetic-private-cookie");
  if (family === "gemini-web") expect(request.secretPatches).toContainEqual(expect.objectContaining({ operation: "replace", value: "synthetic-private-cookie" }));
  expect(api.getGeminiAuthSession).not.toHaveBeenCalled(); expect(api.completeGeminiAuthSession).not.toHaveBeenCalled();
  expect(api.probeProvider).not.toHaveBeenCalled(); expect(api.probeCredential).not.toHaveBeenCalled();
  if (family === "flow-web") {
    expect(request.document.model_routes).toEqual([]);
    expect((request.document.providers[0] as Record<string, unknown>).credentials).toContainEqual(expect.objectContaining({
      id: "json-account", supported_models: [], runtime_state_object_key: "credential-runtime/json-test/http-cookies.json",
    }));
  }
  if (family === "stitch-web") {
    expect(request.document.model_routes).toContainEqual(expect.objectContaining({ pattern: "stitch-design", provider_ids: ["custom-pool"] }));
    expect((request.document.providers[0] as Record<string, unknown>).credentials).toContainEqual(expect.objectContaining({
      id: "json-account", supported_models: ["stitch-design"], model_selection: { fetched: [], manual: ["stitch-design"] },
    }));
  }
});
