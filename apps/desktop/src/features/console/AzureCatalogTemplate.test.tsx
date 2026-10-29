// Azure deployment names are resource-owned, not entries from the public model catalog.
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";

const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "azure-api")!;
const baseUrl = "https://fixture-resource.openai.azure.com/openai/v1";
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates an empty azure-api pool while retaining Azure authentication and runtime ownership", () => {
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, {
    templateId: template.id, providerId: template.providerId, providerLabel: template.providerId,
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl, supportedModels: [],
    workflow: defaultCatalogWorkflow(template),
  });
  expect(result.providers[0]).toMatchObject({ id: "azure-api", preset: "azure-api", base_url: baseUrl,
    explicit_credentials_only: true, credentials: [], supported_models: [], test_policy: { automaticEnabled: false } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(catalogProviderKind(result.providers[0] as Record<string, unknown>)).toBeUndefined();
});

it("imports an explicit deployment without sending the Azure key to the OpenAI catalog or probing models", () => {
  const onSubmit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "azure-api", label: "Azure" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind({ preset: "azure-api", base_url: baseUrl })}
    onDiscoverProviderModels={official} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "azure-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "my-deployment" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "azure-api", supportedModelsText: "my-deployment" }));
  expect(official).not.toHaveBeenCalled();
  expect(generic).not.toHaveBeenCalled();
});
