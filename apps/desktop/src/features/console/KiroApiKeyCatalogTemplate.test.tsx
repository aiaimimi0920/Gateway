// The CLI subscription Key is a separate static product, not the existing OAuth form or directory.
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow, catalogWorkflowError } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";
import { supportsAccountDiscovery } from "./accountDiscovery";

const baseUrl = "https://runtime.us-east-1.kiro.dev";
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function keyTemplate() {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "kiro-api-key");
  expect(template).toBeDefined(); return template!;
}
it("creates an empty static Key pool without models, credentials, aliases or automatic tests", () => {
  const template = keyTemplate(); const workflow = defaultCatalogWorkflow(template);
  expect(catalogWorkflowError(workflow)).toBeNull();
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, {
    templateId: template.id, providerId: template.providerId, providerLabel: template.providerId,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl, supportedModels: [], workflow,
  });
  expect(template.providerId).toBe("kiro-api");
  expect(result.providers[0]).toMatchObject({ id: "kiro-api", preset: "kiro-api-key", base_url: baseUrl,
    explicit_credentials_only: true, credentials: [], supported_models: [], test_policy: { automaticEnabled: false, models: [] } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).not.toHaveProperty("default_model");
  expect(result.providers[0]).not.toHaveProperty("kiro_oauth");
  expect(result.model_routes).toEqual([]);
  expect(catalogProviderKind(result.providers[0] as Record<string, unknown>)).toBeUndefined();
  expect(supportsAccountDiscovery(result.providers[0] as Record<string, unknown>)).toBe(false);
  expect(() => addCatalogProvider(result, template, { templateId: template.id, providerId: "other", providerLabel: "other",
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: "https://untrusted.invalid", supportedModels: [] })).toThrow("固定");
});
it("offers a fixed runtime template, allocates repeated instances and does not request secret access", () => {
  const submit = vi.fn(); const secret = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={["kiro-api", "kiro-api-2"]}
    existingCredentialIds={[]} locked={false} hasSecretAccess={false} onOpenChange={vi.fn()}
    onRequestSecretAccess={secret} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^kiro-api/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue(baseUrl);
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "kiro-api-key", providerId: "kiro-api-3", baseUrl }));
  expect(secret).not.toHaveBeenCalled();
});
it("keeps exact manual model IDs and never consumes a Key for generic or official discovery", () => {
  const template = keyTemplate(); const submit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "custom-pool", label: template.labelZh }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={submit} catalogProviderKind={() => catalogProviderKind({ preset: template.preset, base_url: baseUrl })}
    onDiscoverProviderModels={official} onDiscover={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "stable-key" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "Exact.Model/Version" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "custom-pool", credentialId: "stable-key",
    supportedModelsText: "Exact.Model/Version", apiKeyOperation: "replace", apiKeyValue: "synthetic-key" }));
  expect(official).not.toHaveBeenCalled(); expect(generic).not.toHaveBeenCalled();
});
it("rejects a programmatically changed runtime target before creating the pool", () => {
  const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]} locked={false}
    hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^kiro-api/ }));
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: "https://untrusted.invalid" } });
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(screen.getByText("Kiro CLI API Key 仅支持固定的 US East runtime 地址。")).toBeInTheDocument();
  expect(submit).not.toHaveBeenCalled();
});
