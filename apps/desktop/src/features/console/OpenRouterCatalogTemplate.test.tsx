import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";

const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "openrouter-openai")!;
const model = "openrouter/free";

it("creates an empty OpenRouter pool with the canonical official URL and locked adapters", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: /^openrouter/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://openrouter.ai/api/v1");
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) {
    expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  }
  expect(screen.getByLabelText("调用适配器")).toHaveValue("OpenAI API");
  expect(screen.getByLabelText("指纹适配器")).toHaveValue("自动");
  expect(screen.getByLabelText("专用适配器")).toHaveValue("无（标准 API）");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("tab", { name: "站点健康" }));
  expect(screen.getByLabelText("适配器")).toHaveValue("openrouter-status");
  expect(screen.getByLabelText("适配器")).toHaveAttribute("readonly");
  expect(screen.getByLabelText("监控站地址")).toHaveValue("https://status.openrouter.ai/");
  expect(screen.getByLabelText("监控站地址")).toHaveAttribute("readonly");
  await user.click(screen.getByRole("button", { name: "添加" }));
  const draft = onSubmit.mock.calls[0][0] as ProviderCatalogDraft;
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(result.providers).toEqual([expect.objectContaining({ id: "openrouter-api", preset: "openrouter-openai",
    base_url: "https://openrouter.ai/api/v1", explicit_credentials_only: true, credentials: [],
    supported_models: [model], call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
  })]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.model_routes).toEqual([{ pattern: model, provider_ids: ["openrouter-api"] }]);
});

it("preserves existing providers, credentials and routes when adding OpenRouter", () => {
  const previous = { id: "existing", credentials: [{ id: "fixture-account", api_key: "fixture-only" }] };
  const document = { providers: [previous], model_routes: [{ pattern: model, provider_ids: ["existing"] }], aliases: {} };
  const before = structuredClone(document);
  const draft: ProviderCatalogDraft = { templateId: template.id, providerId: "openrouter", providerLabel: "openrouter",
    vendorKey: template.vendorKey, vendorName: template.vendorName, baseUrl: template.baseUrl,
    supportedModels: [...template.supportedModels], workflow: defaultCatalogWorkflow(template) };
  const result = addCatalogProvider(document, template, draft);
  expect(document).toEqual(before);
  expect(result.providers[0]).toEqual(previous);
  expect(result.providers[1]).toMatchObject({ explicit_credentials_only: true, credentials: [] });
  expect(result.model_routes).toEqual([{ pattern: model, provider_ids: ["existing", "openrouter"] }]);
});
