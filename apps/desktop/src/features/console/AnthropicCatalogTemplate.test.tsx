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

const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "anthropic")!;
const catalog = { sourceUrl: "https://api.anthropic.com/v1/models?limit=1000" as const,
  checkedAt: "2026-10-27T00:00:00Z", models: ["claude-sonnet-4-6", "another-model"] };
const document = { providers: [{ id: "anthropic-api", preset: "anthropic", base_url: "https://api.anthropic.com",
  credentials: [{ id: "one", supported_models: [], model_selection: { fetched: [], manual: ["custom/model"] } }] }], model_routes: [], aliases: {} };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates only an empty Anthropic pool with locked official adapters and a bounded opt-in test", async () => {
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: /^anthropic-api(?:\s*新调用流程已验收)?$/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://api.anthropic.com");
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0] as ProviderCatalogDraft);
  expect(result.providers[0]).toMatchObject({ preset: "anthropic", base_url: "https://api.anthropic.com",
    explicit_credentials_only: true, credentials: [], supported_models: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
    test_policy: { automaticEnabled: false, modelSelection: "selected", models: ["claude-sonnet-4-6"],
      cases: [{ expectedAnswer: null, enabled: true }] } });
  expect(result.model_routes).toEqual([]);
  expect(result.providers[0]).toHaveProperty("site_health");
  expect(result.providers[0]).not.toHaveProperty("api_key");
});

it("imports through catalog-only discovery rather than bulk generation", async () => {
  const discover = vi.fn().mockResolvedValue(catalog); const onSubmit = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "anthropic-api", label: "Anthropic" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind(document.providers[0])}
    onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(discover).toHaveBeenCalledWith({ providerId: "anthropic-api", apiKey: "fixture-key" }, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
});

it("refreshes an exact account, preserving manual models without verified-model evidence", async () => {
  const opts = { client: { request: vi.fn().mockResolvedValue({ revision: "r1", catalog }) } as unknown as GatewayApiClient,
    managementToken: "fixture", secretGrant: "grant", editorText: JSON.stringify(document), revision: "r1", draftDirty: false,
    replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
  const hook = renderHook(() => useAccountDiscovery(opts));
  await act(() => hook.result.current.refreshAccountDiscovery("anthropic-api", "one"));
  expect(opts.client.request).toHaveBeenCalledWith("/v1/internal/gateway/console/anthropic-model-catalog", expect.anything(),
    expect.objectContaining({ body: { credentialId: "one" } }));
  const saved = opts.replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0].credentials[0]).toMatchObject({ model_catalog: catalog,
    model_selection: { fetched: catalog.models, manual: ["custom/model"] }, supported_models: ["custom/model", ...catalog.models] });
  expect(saved.model_routes).toHaveLength(3);
  expect(JSON.stringify(saved)).not.toContain("verified_models");
  expect(applyOfficialCatalog(saved, "anthropic-api", "one", catalog)).toEqual(saved);
  for (const base of ["https://api.anthropic.com", "https://api.anthropic.com/"]) expect(officialCatalogBase("anthropic", base)).toBe(true);
  for (const base of ["https://api.anthropic.com/v1", "http://api.anthropic.com", "https://api.anthropic.com.evil.invalid/v1", "https://api.anthropic.com?key=x"])
    expect(officialCatalogBase("anthropic", base)).toBe(false);
  expect(() => applyOfficialCatalog(document, "anthropic-api", "one", { ...catalog, sourceUrl: "https://api.poe.com/v1/models" })).toThrow();
});

it("leaves a failed catalog import open and never falls back to generation", async () => {
  const discover = vi.fn().mockRejectedValue(new Error("Cannot read official model catalog; previous catalog is unchanged."));
  const onSubmit = vi.fn(); const onOpenChange = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "anthropic-api", label: "Anthropic" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={onOpenChange} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind(document.providers[0])}
    onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText(/Cannot read official model catalog/);
  expect(onSubmit).not.toHaveBeenCalled(); expect(onOpenChange).not.toHaveBeenCalled(); expect(generic).not.toHaveBeenCalled();
});
