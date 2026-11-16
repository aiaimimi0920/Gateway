// Express Key is a manual-model product, not the AI Studio catalogue or full Vertex OAuth.
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";
import { supportsAccountDiscovery } from "./accountDiscovery";

const baseUrl = "https://aiplatform.googleapis.com/v1/publishers/google";
const provider = { id: "renamed", preset: "vertex-express-api", base_url: baseUrl };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function template() {
  const result = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "vertex-express-api");
  expect(result).toBeDefined(); return result!;
}
it("creates a separate empty Key pool without a guessed model, OAuth or catalogue", () => {
  const entry = template();
  const draft = { templateId: entry.id, providerId: entry.providerId, providerLabel: entry.providerId,
    vendorKey: entry.vendorKey, vendorName: entry.vendorName, baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(entry) };
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, draft);
  expect(result.providers[0]).toMatchObject({ id: "vertex-express-api", preset: "vertex-express-api", base_url: baseUrl,
    explicit_credentials_only: true, credentials: [], supported_models: [], test_policy: { automaticEnabled: false, models: [] },
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).not.toHaveProperty("default_model");
  expect(result.model_routes).toEqual([]);
  expect(catalogProviderKind(provider)).toBeUndefined(); expect(supportsAccountDiscovery(provider)).toBe(false);
  expect(() => addCatalogProvider(result, entry, { ...draft, providerId: "other", baseUrl: "https://untrusted.invalid" })).toThrow("固定");
});
it("offers a read-only fixed target and allocates repeated pools without secret access", () => {
  const submit = vi.fn(); const secret = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={["vertex-express-api"]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={secret} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^vertex-express-api/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue(baseUrl);
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "vertex-express-api", providerId: "vertex-express-api-2", baseUrl }));
  expect(secret).not.toHaveBeenCalled();
});
it("refuses a programmatically changed Express target before creating a pool", () => {
  const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]} locked={false}
    hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^vertex-express-api/ }));
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "https://untrusted.invalid" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled(); expect(screen.getByText(/Vertex Express.*固定/)).toBeInTheDocument();
});
it("uses the ordinary Key field and exact manual model without any directory calls", () => {
  const submit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "renamed", label: "Express" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={submit} onDiscoverProviderModels={official} onDiscover={generic}
    catalogProviderKind={() => catalogProviderKind(provider)} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "stable" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-only" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "gemini-contract-001" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "renamed", credentialId: "stable",
    apiKeyOperation: "replace", apiKeyValue: "synthetic-only", supportedModelsText: "gemini-contract-001" }));
  expect(official).not.toHaveBeenCalled(); expect(generic).not.toHaveBeenCalled();
});
