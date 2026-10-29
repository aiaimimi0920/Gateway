import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";

it("replaces the two Jina entries with one empty pool and two ungraded operation tests", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const templates = PROVIDER_CATALOG_TEMPLATES.filter(({ vendorKey }) => vendorKey === "jina-ai");
  expect(templates.map(({ id }) => id)).toEqual(["jina"]);
  const template = templates[0];
  const onSubmit = vi.fn();
  const user = userEvent.setup();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: /^jina-api(?:\s*新调用流程已验收)?$/ }));
  expect(screen.queryByRole("button", { name: /^jina-search/ })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /^jina-reader/ })).not.toBeInTheDocument();
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://s.jina.ai");
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) {
    expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  }
  expect(screen.getByLabelText("调用适配器")).toHaveValue("搜索 API");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("首个账号 ID")).not.toBeInTheDocument();
  await user.click(screen.getByRole("tab", { name: "站点健康" }));
  expect(screen.getByLabelText("适配器")).toHaveValue("jina-status");
  await user.click(screen.getByRole("button", { name: "添加" }));
  const draft = onSubmit.mock.calls[0][0] as ProviderCatalogDraft;
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(result.providers).toEqual([expect.objectContaining({ id: "jina-api", preset: "jina",
    base_url: "https://s.jina.ai", explicit_credentials_only: true, credentials: [],
    supported_models: ["jina-search", "jina-fetch", "search", "fetch"],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
    test_policy: expect.objectContaining({ automaticEnabled: false, modelSelection: "selected",
      models: ["jina-search", "jina-fetch"], cases: [expect.objectContaining({
        prompt: "https://example.com/", expectedAnswer: null, enabled: true,
      })] }),
  })]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.model_routes).toEqual(template.supportedModels.map((pattern) => ({ pattern, provider_ids: ["jina-api"] })));
});
