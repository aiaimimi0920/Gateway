// 仅使用假凭据验证正常手动导入，禁止测试触发真实上游请求。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("creates an empty deepseek-api pool with one opt-in model and no automatic tests", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "deepseek-api");
  expect(template).toBeDefined();
  expect(template).toMatchObject({ providerId: "deepseek-api", preset: "deepseek-api", baseUrl: "https://api.deepseek.com", supportedModels: [] });
  expect(PROVIDER_CATALOG_TEMPLATES.some(({ id }) => id === "deepseek-openai")).toBe(false);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
    templateId: template!.id, providerId: template!.providerId, providerLabel: template!.providerId,
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [], workflow: defaultCatalogWorkflow(template!),
  });
  expect(result.providers[0]).toMatchObject({ id: "deepseek-api", preset: "deepseek-api", explicit_credentials_only: true,
    credentials: [], supported_models: [], test_policy: { automaticEnabled: false, modelSelection: "selected", models: ["deepseek-flash"] } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
});
it("imports only the manually declared model without discovery or probing", () => {
  const onSubmit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "deepseek-api", label: "DeepSeek" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind({ preset: "deepseek-api", base_url: "https://api.deepseek.com" })}
    onDiscoverProviderModels={official} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "deepseek-fixture" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "deepseek-flash" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "deepseek-api", supportedModelsText: "deepseek-flash" }));
  expect(official).not.toHaveBeenCalled(); expect(generic).not.toHaveBeenCalled();
});
