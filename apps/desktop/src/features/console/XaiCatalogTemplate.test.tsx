// 仅本地假凭据：不调用真实目录或生成接口。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("creates an empty xai-api pool without automatic or directory-wide tests", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "xai-api");
  expect(template).toBeDefined();
  expect(template).toMatchObject({ providerId: "xAI-api", preset: "xai-api", baseUrl: "https://api.x.ai/v1", supportedModels: [] });
  expect(PROVIDER_CATALOG_TEMPLATES.some(({ id }) => id === "xai-openai")).toBe(false);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
    templateId: template!.id, providerId: template!.providerId, providerLabel: template!.providerId,
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [...template!.supportedModels], workflow: defaultCatalogWorkflow(template!),
  });
  expect(result.providers[0]).toMatchObject({ id: "xAI-api", preset: "xai-api", explicit_credentials_only: true,
    credentials: [], supported_models: [], test_policy: { automaticEnabled: false, modelSelection: "selected", models: ["grok-4.7"] } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
});
it("imports a manually selected model without sending the key to another catalog or probing", () => {
  const onSubmit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "xAI-api", label: "xAI" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind({ preset: "xai-api", base_url: "https://api.x.ai/v1" })}
    onDiscoverProviderModels={official} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "xai-fixture" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "grok-4.7" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "xAI-api", supportedModelsText: "grok-4.7" }));
  expect(official).not.toHaveBeenCalled(); expect(generic).not.toHaveBeenCalled();
});
