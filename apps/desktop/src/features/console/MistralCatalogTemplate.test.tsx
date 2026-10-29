// 无真实凭据：验证空池与手动模型导入，不执行上游模型探测。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates an empty mistral-api pool with no seeded models or automatic tests", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "mistral-api");
  expect(template).toBeDefined();
  expect(template).toMatchObject({ providerId: "mistral-api", preset: "mistral-api",
    baseUrl: "https://api.mistral.ai/v1", supportedModels: [] });
  expect(PROVIDER_CATALOG_TEMPLATES.some(({ id }) => id === "mistral-openai")).toBe(false);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
    templateId: template!.id, providerId: template!.providerId, providerLabel: template!.providerId,
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [...template!.supportedModels], workflow: defaultCatalogWorkflow(template!),
  });
  expect(result.providers[0]).toMatchObject({ id: "mistral-api", preset: "mistral-api",
    explicit_credentials_only: true, credentials: [], supported_models: [],
    test_policy: { automaticEnabled: false, modelSelection: "selected", models: ["mistral-large-latest"] } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
});

it("imports a manual Mistral model without sending the key to another catalog or probing", () => {
  const onSubmit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "mistral-api", label: "Mistral" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind({ preset: "mistral-api", base_url: "https://api.mistral.ai/v1" })}
    onDiscoverProviderModels={official} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "mistral-fixture" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "mistral-large-latest" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "mistral-api", supportedModelsText: "mistral-large-latest" }));
  expect(official).not.toHaveBeenCalled();
  expect(generic).not.toHaveBeenCalled();
});
