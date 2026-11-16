import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { applyOfficialCatalog, catalogProviderKind, officialCatalogBase } from "./officialModelCatalog";
import type { OfficialModelCatalog } from "./officialModelCatalog";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

const base = "https://api.commandcode.ai/provider/v1";
const catalog = { sourceUrl: `${base}/models`, checkedAt: "2026-11-09T00:00:00Z", models: ["commandcode-live-text"] };
const decisionCatalog: OfficialModelCatalog = { ...catalog, models: ["jev-synthetic"],
  supportedEndpoints: { "jev-synthetic": ["/systemone"] } };
const provider = { id: "commandcode", preset: "commandcode-api", base_url: base,
  credentials: [{ id: "one", supported_models: [], model_selection: { fetched: [], manual: ["commandcode-manual"] } }] };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("creates an empty CommandCode Provider API Key pool without guessed models or live acceptance", async () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "commandcode-api");
  expect(template).toBeDefined();
  expect(templateAdaptationComplete("commandcode-api")).toBe(false);
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "commandcode-api" }));
  expect(screen.getByLabelText("Base URL")).toHaveValue(base);
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template!, onSubmit.mock.calls[0][0]);
  expect(result.providers[0]).toMatchObject({ preset: "commandcode-api", base_url: base,
    explicit_credentials_only: true, credentials: [], supported_models: [],
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" },
    test_policy: { automaticEnabled: false, models: [] } });
  expect(result.providers[0]).not.toHaveProperty("api_key");
  expect(result.model_routes).toEqual([]);
});

it.each([catalog, decisionCatalog])("uses account catalog reads rather than generation probes and preserves manual choices: $models", async (catalog) => {
  expect(catalogProviderKind(provider)).toBe("commandcode");
  const discover = vi.fn().mockResolvedValue(catalog); const onSubmit = vi.fn(); const generic = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "commandcode", label: "CommandCode" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => catalogProviderKind(provider)} onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "cpk-synthetic" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(discover).toHaveBeenCalledWith({ providerId: "commandcode", apiKey: "cpk-synthetic" }, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
  const saved = applyOfficialCatalog({ providers: [provider], model_routes: [], aliases: {} }, "commandcode", "one", catalog);
  expect(saved.providers[0]).toMatchObject({ credentials: [{ model_catalog: catalog,
    model_selection: { fetched: catalog.models, manual: ["commandcode-manual"] }, supported_models: ["commandcode-manual", ...catalog.models] }] });
  expect(JSON.stringify(saved)).not.toContain("verified_models");
});

it("rejects console, Raccoon, plaintext and lookalike origins for the Provider API pool", () => {
  expect(officialCatalogBase("commandcode", base)).toBe(true);
  for (const bad of ["https://platform.commandcode.cn", "https://xiaohuanxiong.com", "http://token.commandcode.cn/v1", `${base}?key=x`, "https://token.commandcode.cn.evil.invalid/v1"]) {
    expect(officialCatalogBase("commandcode", bad)).toBe(false);
  }
  expect(() => applyOfficialCatalog({ providers: [provider], model_routes: [], aliases: {} }, "commandcode", "one", {
    ...catalog, sourceUrl: "https://api.river.ai/v1/models" })).toThrow();
});

it("keeps a failed catalog read open without saving the new Key", async () => {
  const discover = vi.fn().mockRejectedValue(new Error("Synthetic CommandCode catalog failure"));
  const onSubmit = vi.fn(); const onOpenChange = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "commandcode", label: "CommandCode" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={onOpenChange} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => "commandcode"} onDiscoverProviderModels={discover} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "cpk-synthetic" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await screen.findByText("Synthetic CommandCode catalog failure");
  expect(onSubmit).not.toHaveBeenCalled();
  expect(onOpenChange).not.toHaveBeenCalled();
});

it("offers account ZDR as an opt-in and includes it in the saved draft", async () => {
  const submit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "commandcode", label: "CommandCode" }]}
    initialValue={{ providerId: "commandcode", credentialId: "one", accountName: "", enabled: true, baseUrl: "", supportedModelsText: "manual", apiKeyOperation: "keep", apiKeyValue: "" }}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={submit} catalogProviderKind={() => "commandcode"} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  const control = screen.getByLabelText("零数据保留（ZDR）");
  expect(control).toHaveValue("inherit");
  fireEvent.change(control, { target: { value: "required" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(submit).toHaveBeenCalledWith(expect.objectContaining({ commandcodeZdr: "required", apiKeyOperation: "keep" })));
});
