import { fireEvent, render, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

it("creates an isolated empty Luma inference pool without media models or automatic paid probes", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(t => t.id === "lumalabs-inference-api");
  expect(template).toBeDefined();
  const workflow = defaultCatalogWorkflow(template!);
  const draft = { templateId: template!.id, providerId: "my-inference", providerLabel: "LLM",
    vendorKey: template!.vendorKey, vendorName: template!.vendorName, baseUrl: template!.baseUrl,
    supportedModels: [], workflow } satisfies ProviderCatalogDraft;
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, draft);
  expect(result.providers[0]).toMatchObject({ id: "my-inference", preset: "lumalabs-inference-api",
    base_url: "https://inference.lumalabs.ai/v1", explicit_credentials_only: true, credentials: [],
    supported_models: [], test_policy: { automaticEnabled: false, models: [] },
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).not.toHaveProperty("site_health");
  expect(result.model_routes).toEqual([]);
  expect(templateAdaptationComplete(template!.id)).toBe(false);
  expect(template!.vendorKey).not.toBe(PROVIDER_CATALOG_TEMPLATES.find(t => t.id === "lumalabs-api")!.vendorKey);
});

it("offers the fixed inference target separately from Agents media and MCP", () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: "lumalabs-inference-api" }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://inference.lumalabs.ai/v1");
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledTimes(1);
  expect(onSubmit.mock.calls[0][0].workflow.testPolicy.automaticEnabled).toBe(false);
});
