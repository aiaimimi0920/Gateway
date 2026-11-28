import { expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { catalogFirstAccountSecret, supportsCatalogApiKey } from "./catalogFirstAccount";
import { applyOfficialCatalog, catalogProviderKind } from "./officialModelCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";

it("selects ZenMux in the directory and submits a Key through the catalog-only account form", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const submit = vi.fn();
  const view = render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit}/></UiLocaleProvider>, {wrapper: NeuroProvider});
  fireEvent.click(screen.getByRole("button", {name: "zenmux-api"}));
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://zenmux.ai/api/v1");
  fireEvent.click(screen.getByRole("checkbox", {name: "同时添加首个账号"}));
  fireEvent.click(screen.getByRole("button", {name: "添加"}));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({templateId: "zenmux-api", supportedModels: []}));
  view.unmount();
  const catalog = {sourceUrl: "https://zenmux.ai/api/v1/models", checkedAt: "2026-11-28T00:00:00Z", models: ["org/Test"]};
  const discover = vi.fn().mockResolvedValue(catalog), generic = vi.fn(), save = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{id: "zenmux-api", label: "ZenMux"}]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={save} catalogProviderKind={() => catalogProviderKind({preset: "zenmux-api"})}
    onDiscoverProviderModels={discover} onDiscover={generic}/></UiLocaleProvider>, {wrapper: NeuroProvider});
  fireEvent.change(screen.getByLabelText("账号 ID"), {target: {value: "one"}});
  fireEvent.change(screen.getByLabelText("API Key"), {target: {value: "synthetic-only"}});
  fireEvent.click(screen.getByRole("button", {name: "保存"}));
  await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({modelCatalog: catalog})));
  expect(discover).toHaveBeenCalledWith({providerId: "zenmux-api", apiKey: "synthetic-only"}, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
});

it("creates an empty ZenMux pool, keeps the Key out of the route draft, then routes exact catalog IDs", () => {
  const t = PROVIDER_CATALOG_TEMPLATES.find(t => t.id === "zenmux-api")!;
  const draft: ProviderCatalogDraft = { templateId: t.id, providerId: t.providerId, providerLabel: t.labelEn,
    vendorKey: t.vendorKey, vendorName: t.vendorName, baseUrl: t.baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(t) };
  const empty = { providers: [], model_routes: [], aliases: {} };
  expect(supportsCatalogApiKey(t)).toBe(true);
  const pool = addCatalogProvider(empty, t, draft);
  expect(pool.providers[0]).toMatchObject({ credentials: [], supported_models: [], test_policy: { automaticEnabled: false, models: [] } });
  expect(pool.model_routes).toEqual([]);
  const first = { ...draft, credentialId: "one", apiKey: "synthetic-only" };
  const withAccount = addCatalogProvider(empty, t, first);
  expect(JSON.stringify(withAccount)).not.toContain(first.apiKey);
  expect(catalogFirstAccountSecret(first)).toMatchObject({ field: "api_key", value: first.apiKey });
  expect(catalogProviderKind({preset: t.preset})).toBe("zenmux");
  const catalog = {sourceUrl: "https://zenmux.ai/api/v1/models", checkedAt: "2026-11-28T00:00:00Z", models: ["vendor/Exact"]};
  const saved = applyOfficialCatalog(withAccount, t.providerId, "one", catalog);
  expect(saved.model_routes).toContainEqual(expect.objectContaining({pattern: "vendor/Exact", provider_ids: [t.providerId]}));
  expect(() => applyOfficialCatalog(withAccount, t.providerId, "one", {...catalog, sourceUrl: "https://api.openai.com/v1/models"})).toThrow();
});
