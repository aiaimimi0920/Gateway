import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { applyOfficialCatalog, catalogProviderKind, officialCatalogBase } from "./officialModelCatalog";
import { useAccountDiscovery } from "./useAccountDiscovery";
import { TEMPLATE_ADAPTATION_RECEIPTS, templateAdaptationComplete } from "./providerTemplateAdaptation";

const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "cohere-chat")!;
const catalog = { sourceUrl: "https://api.cohere.com/v1/models?endpoint=chat&page_size=1000" as const,
  checkedAt: "2026-10-27T00:00:00Z", models: ["command-r7b-12-2024", "command-a-03-2025"] };
const document = { providers: [{ id: "cohere", preset: "cohere-chat", base_url: "https://api.cohere.com",
  credentials: [{ id: "one", supported_models: [], model_selection: { fetched: [], manual: ["custom-model"] } }] }], model_routes: [], aliases: {} };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("records completion only after exact-account UI and four-mode live acceptance", () => {
  expect(templateAdaptationComplete("cohere-chat")).toBe(true);
  expect(TEMPLATE_ADAPTATION_RECEIPTS.find(({ templateId }) => templateId === "cohere-chat")).toMatchObject({
    evidencePath: "docs/progress/2026-10-27-cohere-template-acceptance.md", verifiedAt: "2026-10-26T20:43:42Z", flowVersion: 1,
  });
});

it("creates an empty Cohere pool with locked native adapters, site health and one opt-in test", async () => {
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: /^cohere-api(?:\s*新调用流程已验收)?$/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.cohere.com");
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0] as ProviderCatalogDraft);
  expect(result.providers[0]).toMatchObject({ preset: "cohere-chat", base_url: "https://api.cohere.com",
    explicit_credentials_only: true, credentials: [], supported_models: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
    site_health: { enabled: true, adapter: "cohere-status" },
    test_policy: { automaticEnabled: false, modelSelection: "selected", models: ["command-r7b-12-2024"],
      cases: [{ expectedAnswer: null, enabled: true }] } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).not.toHaveProperty("api_key");
});

it("imports Cohere through metadata-only discovery without generic generation", async () => {
  const discover = vi.fn().mockResolvedValue(catalog); const onSubmit = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "cohere", label: "Cohere" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind(document.providers[0])}
    onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(discover).toHaveBeenCalledWith({ providerId: "cohere", apiKey: "fixture-key" }, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
});

it("refreshes the exact Cohere account while preserving manual models and official origin binding", async () => {
  const opts = { client: { request: vi.fn().mockResolvedValue({ revision: "r1", catalog }) } as unknown as GatewayApiClient,
    managementToken: "fixture", secretGrant: "grant", editorText: JSON.stringify(document), revision: "r1", draftDirty: false,
    replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("cohere", "one"));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/cohere-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "one" } }));
  const saved = opts.replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0].credentials[0]).toMatchObject({ model_catalog: catalog,
    model_selection: { fetched: catalog.models, manual: ["custom-model"] }, supported_models: ["custom-model", ...catalog.models] });
  expect(saved.model_routes).toHaveLength(3);
  expect(JSON.stringify(saved)).not.toContain("verified_models");
  expect(applyOfficialCatalog(saved, "cohere", "one", catalog)).toEqual(saved);
  expect(officialCatalogBase("cohere", "https://api.cohere.com/")).toBe(true);
  for (const base of ["https://api.cohere.com/v2", "http://api.cohere.com", "https://api.cohere.com.evil.invalid", "https://api.cohere.com?key=x"])
    expect(officialCatalogBase("cohere", base)).toBe(false);
  expect(() => applyOfficialCatalog(document, "cohere", "one", { ...catalog, sourceUrl: "https://api.poe.com/v1/models" })).toThrow();
});
