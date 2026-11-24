// 无真实凭据：验证空池与手动模型导入，不执行上游模型探测。
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("submits the default Vibe template without a key or a test model", () => {
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: "mistral-vibe-api" }));
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledOnce();
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    templateId: "mistral-vibe-api", providerId: "mistral-vibe-api", supportedModels: [],
    workflow: expect.objectContaining({
      testPolicy: expect.objectContaining({ automaticEnabled: false, modelSelection: "all", models: [] }),
    }),
  }));
  expect(onSubmit.mock.calls[0][0].credentialId).toBeUndefined();
});

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
        automaticEnabled: false, modelSelection: id === "mistral-studio-api" ? "selected" : "all",
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

it.each(["mistral-studio-api", "mistral-vibe-api"])("refreshes the saved %s account using its official catalog", async (id) => {
  const catalog = { sourceUrl: "https://api.mistral.ai/v1/models", checkedAt: "2026-11-24T00:00:00Z", models: ["mistral-medium-3.5"] };
  const official = vi.fn().mockResolvedValue(catalog);
  const generic = vi.fn();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id, label: "Mistral" }]}
    initialValue={{ providerId: id, credentialId: "mistral-saved", accountName: "", enabled: true,
      baseUrl: "", supportedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "" }}
    existingCredentialIds={["mistral-saved"]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind({ preset: id, base_url: "https://api.mistral.ai/v1" })}
    onDiscoverProviderModels={official} onDiscoverAccountModels={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: "拉取模型" }));
  await waitFor(() => expect(screen.getByLabelText("拉取模型列表")).toHaveValue("mistral-medium-3.5"));
  expect(official).toHaveBeenCalledWith({ credentialId: "mistral-saved" }, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "mistral-saved", modelCatalog: catalog }));
});
