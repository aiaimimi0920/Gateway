// 无真实凭据：验证空池与手动模型导入，不执行上游模型探测。
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates separate empty Studio and Vibe pools without automatic tests", () => {
  let result = { providers: [], model_routes: [], aliases: {} } as Parameters<typeof addCatalogProvider>[0];
  for (const id of ["mistral-studio-api", "mistral-vibe-api"]) {
    const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === id)!;
    expect(template).toMatchObject({
      providerId: id, preset: id, baseUrl: "https://api.mistral.ai/v1", supportedModels: [],
    });
    result = addCatalogProvider(result, template, {
      templateId: id, providerId: id, providerLabel: id,
      vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
      supportedModels: [], workflow: defaultCatalogWorkflow(template),
    });
    const pool = result.providers.at(-1);
    expect(pool).toMatchObject({
      id, preset: id, explicit_credentials_only: true, credentials: [], supported_models: [],
      test_policy: {
        automaticEnabled: false, modelSelection: "selected",
        models: id === "mistral-studio-api" ? ["mistral-large-latest"] : [],
      },
    });
    expect(pool).not.toHaveProperty("api_key");
  }
  expect(result.providers).toMatchObject([{ id: "mistral-studio-api" }, { id: "mistral-vibe-api" }]);
  expect(result.model_routes).toEqual([]);
});

it.each(["mistral-api", "mistral-studio-api", "mistral-vibe-api"])("imports a manual %s model without probing", (id) => {
  const onSubmit = vi.fn(); const official = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id, label: "Mistral" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind({ preset: id, base_url: "https://api.mistral.ai/v1" })}
    onDiscoverProviderModels={official} onDiscover={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "mistral-fixture" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "mistral-large-latest" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: id, supportedModelsText: "mistral-large-latest" }));
  expect(official).not.toHaveBeenCalled();
  expect(generic).not.toHaveBeenCalled();
});
