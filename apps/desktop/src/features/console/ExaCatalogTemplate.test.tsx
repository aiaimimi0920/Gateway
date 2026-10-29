import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";

const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "exa")!;

it("creates an empty Exa pool with fixed adapters and search-only ungraded tests", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: /^exa/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.exa.ai");
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) {
    expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  }
  expect(screen.getByLabelText("调用适配器")).toHaveValue("搜索 API");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("tab", { name: "站点健康" }));
  expect(screen.getByLabelText("适配器")).toHaveValue("exa-status");
  expect(screen.getByLabelText("适配器")).toHaveAttribute("readonly");
  expect(screen.getByLabelText("监控站地址")).toHaveValue("https://status.exa.ai/");
  expect(screen.getByLabelText("监控站地址")).toHaveAttribute("readonly");
  await user.click(screen.getByRole("button", { name: "添加" }));
  const draft = onSubmit.mock.calls[0][0] as ProviderCatalogDraft;
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, draft);
  expect(result.providers).toEqual([expect.objectContaining({ id: "exa-api", preset: "exa",
    base_url: "https://api.exa.ai", explicit_credentials_only: true, credentials: [],
    supported_models: ["exa-search", "exa-fetch", "search", "fetch"], call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
  })]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).toMatchObject({ test_policy: { modelSelection: "selected", models: ["exa-search"], cases: [{
    id: "search", prompt: "Rust programming language official documentation", expectedAnswer: null, enabled: true,
  }] } });
  expect(result.model_routes).toEqual(template.supportedModels.map(pattern => ({ pattern, provider_ids: ["exa-api"] })));
});
