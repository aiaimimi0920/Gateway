// Empty MCP storage is not an inference template or a grant to call tools.
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { isLumaMcpProvider } from "./lumaMcp";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function template() {
  const entry = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "lumalabs-mcp");
  expect(entry).toBeDefined(); return entry!;
}
function draft() {
  const entry = template();
  return { templateId: entry.id, providerId: "lumalabs-mcp", providerLabel: "Luma MCP",
    vendorKey: entry.vendorKey, vendorName: entry.vendorName, baseUrl: entry.baseUrl,
    supportedModels: [] as string[], workflow: defaultCatalogWorkflow(entry) };
}
it("creates only a fixed credential-only empty MCP profile without models, probes or secrets", () => {
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template(), draft());
  expect(result.providers).toEqual([{ id: "lumalabs-mcp", label: "Luma MCP", vendor_key: "lumalabs", vendor_name: "Luma",
    base_url: "https://agents.lumalabs.ai/mcp", adapter: "lumalabs_mcp_readonly", protocol_profile: "lumalabs_mcp_readonly",
    protocol_family: "lumalabs_mcp", auth_mode: "luma_mcp_oauth", explicit_credentials_only: true, credentials: [] }]);
  expect(result.model_routes).toEqual([]);
  expect(isLumaMcpProvider(result.providers[0])).toBe(true);
});
it("rejects a changed endpoint, models and active generic workflow rather than creating inference routes", () => {
  const entry = template();
  for (const patch of [{ baseUrl: "https://untrusted.invalid" }, { supportedModels: ["guessed"] }, { apiKey: "synthetic" }]) {
    expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, { ...draft(), ...patch })).toThrow();
  }
  for (const patch of [{ mappings: { guessed: ["model"] } }, { autoRefill: true }, { autoPrune: true }, { callAdapter: "openai" as const }]) {
    const value = draft(); Object.assign(value.workflow, patch);
    expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, value)).toThrow();
  }
  const value = draft(); value.workflow.testPolicy.automaticEnabled = true;
  expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, value)).toThrow();
});
it("preserves existing Web and Agents accounts and routes without mutation", () => {
  const original = { providers: [{ id: "web", preset: "lumalabs", credentials: [{ id: "old", api_key: "synthetic" }] },
    { id: "api", preset: "lumalabs-api", credentials: [] }], model_routes: [{ pattern: "existing", provider_ids: ["api"] }], aliases: {} };
  const before = structuredClone(original), result = addCatalogProvider(original, template(), draft());
  expect(original).toEqual(before); expect(result.providers.slice(0, 2)).toEqual(before.providers);
  expect(result.model_routes).toEqual(before.model_routes);
  expect(() => addCatalogProvider(result, template(), draft())).toThrow("already exists");
});
it("adds an empty pool from the normal directory without secret access or unsupported configuration tabs", () => {
  const submit = vi.fn(), secret = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={["lumalabs-mcp-oauth"]} existingCredentialIds={[]}
    drivers={[{ id: "existing-automation", mode: "http", providerIds: ["lumalabs-mcp-oauth-2"] }]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={secret} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^lumalabs-mcp/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://agents.lumalabs.ai/mcp");
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "lumalabs-mcp", providerId: "lumalabs-mcp-oauth-2" }));
  expect(submit.mock.calls[0][0].workflow.driverId).toBe("");
  expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template(), submit.mock.calls[0][0])).not.toThrow();
  expect(secret).not.toHaveBeenCalled();
});
it("validates the MCP endpoint before submit and restores ordinary API controls when switching templates", () => {
  const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^lumalabs-mcp/ }));
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "https://untrusted.invalid" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled();
  expect(screen.getByText(/Luma MCP.*固定/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /^lumalabs-api/ }));
  expect(screen.getByRole("tablist")).toBeInTheDocument();
});
