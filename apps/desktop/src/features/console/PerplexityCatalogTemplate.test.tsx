// 假凭据只验证正常导入表单，不触发真实请求。
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("creates an empty perplexity-api pool with a single opt-in Sonar model", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "perplexity-api");
  expect(template).toMatchObject({ providerId: "perplexity-api", labelZh: "perplexity-api", labelEn: "perplexity-api", preset: "perplexity-api", baseUrl: "https://api.perplexity.ai", supportedModels: [] });
  expect(PROVIDER_CATALOG_TEMPLATES.some(({ id }) => id === "perplexity")).toBe(false);
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, {
    templateId: template!.id, providerId: template!.providerId, providerLabel: template!.providerId,
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [], workflow: defaultCatalogWorkflow(template!),
  });
  expect(result.providers[0]).toMatchObject({ id: "perplexity-api", preset: "perplexity-api", explicit_credentials_only: true,
    credentials: [], supported_models: [], test_policy: { automaticEnabled: false, modelSelection: "selected", models: ["sonar"] } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
});
it("imports a manually declared account without discovery or automatic probing", () => {
  const onSubmit = vi.fn(); const discover = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "perplexity-api", label: "Perplexity" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} onDiscover={discover} onDiscoverProviderModels={discover} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "perplexity-fixture" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "pplx-invalid-fixture" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "sonar" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "perplexity-api", supportedModelsText: "sonar" }));
  expect(discover).not.toHaveBeenCalled();
});
