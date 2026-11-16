// A static Agents Key pool is separate from the Web board and never assumes model entitlement.
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";
import { supportsAccountDiscovery } from "./accountDiscovery";
import { providerAccessKind } from "./providerAccessKind";

const baseUrl = "https://agents.lumalabs.ai/v1";
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function template() {
  const entry = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "lumalabs-api");
  expect(entry).toBeDefined(); return entry!;
}
function draft() {
  const entry = template();
  return { templateId: entry.id, providerId: "lumalabs-api", providerLabel: "lumalabs-api",
    vendorKey: entry.vendorKey, vendorName: entry.vendorName, baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(entry) };
}
it("creates an independent empty Agents pool with no guessed model, secret or generation policy", () => {
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template(), draft());
  expect(result.providers[0]).toMatchObject({ id: "lumalabs-api", preset: "lumalabs-api", base_url: baseUrl,
    explicit_credentials_only: true, credentials: [], supported_models: [], test_policy: { automaticEnabled: false, models: [] },
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" } });
  expect(result.providers[0]).not.toHaveProperty("api_key"); expect(result.providers[0]).not.toHaveProperty("default_model");
  expect(result.model_routes).toEqual([]);
  expect(catalogProviderKind({ preset: "lumalabs-api" })).toBeUndefined();
  expect(supportsAccountDiscovery({ preset: "lumalabs-api" })).toBe(false);
  expect(PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "lumalabs-web")?.preset).toBe("lumalabs");
  expect(providerAccessKind({ providerPreset: "lumalabs-api", protocolProfile: "lumalabs_agents_api" })).toBe("api");
  expect(providerAccessKind({ providerPreset: "lumalabs" })).toBe("web");
});
it("keeps explicit alias mappings but refuses a foreign target model or endpoint", () => {
  const entry = template(), value = draft(); value.workflow.mappings = { "my-image": ["uni-1"] };
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, value);
  expect(result.providers[0]).toMatchObject({ model_map_targets: { "my-image": ["uni-1"] } });
  expect(() => addCatalogProvider(result, entry, { ...value, providerId: "other", baseUrl: "https://app.lumalabs.ai" })).toThrow("固定");
  value.workflow.mappings = { "my-image": ["web-only"] };
  expect(() => addCatalogProvider(result, entry, { ...value, providerId: "other" })).toThrow("模型");
});
it("offers a read-only fixed target and repeated pool IDs without requiring secret access", () => {
  const submit = vi.fn(), secret = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={["lumalabs-api"]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={secret} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^lumalabs-api/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue(baseUrl); expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "lumalabs-api", providerId: "lumalabs-api-2", baseUrl }));
  expect(secret).not.toHaveBeenCalled();
});
it("refuses a programmatically changed target before submitting a template", () => {
  const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^lumalabs-api/ }));
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "https://untrusted.invalid" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled(); expect(screen.getByText(/Luma Agents.*固定/)).toBeInTheDocument();
});
