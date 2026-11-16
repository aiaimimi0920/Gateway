import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { applyOfficialCatalog, catalogProviderKind, officialCatalogBase } from "./officialModelCatalog";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

const base = "https://api.river.ai/v1";
const catalog = { sourceUrl: `${base}/models`, checkedAt: "2026-11-08T00:00:00Z", models: ["org/live-model"] };
const provider = { id: "river", preset: "river-api", base_url: base,
  credentials: [{ id: "one", supported_models: [], model_selection: { fetched: [], manual: ["org/manual"] } }] };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates an empty River pool through the new call-flow framework without claiming live acceptance", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "river-api");
  expect(template).toBeDefined();
  expect(templateAdaptationComplete("river-api")).toBe(false);
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "river-api" }));
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  expect(screen.getByLabelText("Base URL")).toHaveValue(base);
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, onSubmit.mock.calls[0][0] as ProviderCatalogDraft);
  expect(result.providers[0]).toMatchObject({ preset: "river-api", base_url: base,
    explicit_credentials_only: true, credentials: [], supported_models: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
    test_policy: { automaticEnabled: false, modelSelection: "all", models: [], cases: [{ expectedAnswer: null }] } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.providers[0]).not.toHaveProperty("site_health");
  expect(result.model_routes).toEqual([]);
});

it("imports only the River directory and retains manual selections without inventing model success", async () => {
  expect(catalogProviderKind(provider)).toBe("river");
  const discover = vi.fn().mockResolvedValue(catalog); const onSubmit = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "river", label: "River" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind(provider)} onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "rv_synthetic" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(discover).toHaveBeenCalledWith({ providerId: "river", apiKey: "rv_synthetic" }, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
  const saved = applyOfficialCatalog({ providers: [provider], model_routes: [], aliases: {} }, "river", "one", catalog);
  expect(saved.providers[0]).toMatchObject({ credentials: [{ model_catalog: catalog,
    model_selection: { fetched: catalog.models, manual: ["org/manual"] }, supported_models: ["org/manual", ...catalog.models] }] });
  expect(JSON.stringify(saved)).not.toContain("verified_models");
  expect(applyOfficialCatalog(saved, "river", "one", catalog)).toEqual(saved);
});

it("rejects foreign catalog origins and keeps a failed directory import open", async () => {
  expect(officialCatalogBase("river", base)).toBe(true);
  expect(officialCatalogBase("river", `${base}/`)).toBe(true);
  for (const bad of ["http://api.river.ai/v1", "https://api.river.ai", "https://api.river.ai.evil.invalid/v1", `${base}?key=x`]) {
    expect(officialCatalogBase("river", bad)).toBe(false);
  }
  expect(() => applyOfficialCatalog({ providers: [provider], model_routes: [], aliases: {} }, "river", "one", {
    ...catalog, sourceUrl: "https://api.openai.com/v1/models" })).toThrow();
  const discover = vi.fn().mockRejectedValue(new Error("Synthetic River directory failure"));
  const onSubmit = vi.fn(); const onOpenChange = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "river", label: "River" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={onOpenChange} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => "river"} onDiscoverProviderModels={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "rv_synthetic" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("Synthetic River directory failure");
  expect(onSubmit).not.toHaveBeenCalled(); expect(onOpenChange).not.toHaveBeenCalled();
});
