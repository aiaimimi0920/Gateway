import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { NeuroProvider } from "@neuro/ui";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { CredentialDialog } from "./CredentialDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { cloudflareBase, cloudflareEndpoint } from "./cloudflareWorkersAi";
import { applyOfficialCatalog, catalogProviderKind } from "./officialModelCatalog";
import { templateAdaptationComplete } from "./providerTemplateAdaptation";

const account = "0123456789abcdef0123456789abcdef";
const base = cloudflareBase(account)!;
const catalog = { sourceUrl: cloudflareEndpoint(base)!.sourceUrl, accountBaseUrl: base,
  checkedAt: "2026-11-08T00:00:00Z", models: ["@cf/meta/llama-contract"] };
const provider = { id: "cf", preset: "cloudflare-workers-ai", base_url: base,
  credentials: [{ id: "one", model_selection: { fetched: [], manual: ["@cf/manual/exact"] } }] };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("requires an explicit Cloudflare Account ID and creates no credential, quota or live-acceptance claims", async () => {
  const onSubmit = vi.fn(); const user = userEvent.setup();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onSubmit={onSubmit} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: "cloudflare-workers-ai" }));
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Cloudflare Account ID"), { target: { value: account } });
  expect(screen.getByLabelText("Base URL")).toHaveValue(base);
  for (const label of ["Base URL", "调用适配器", "指纹适配器", "专用适配器"]) expect(screen.getByLabelText(label)).toHaveAttribute("readonly");
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  const template = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "cloudflare-workers-ai")!;
  const saved = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, template, onSubmit.mock.calls[0][0]);
  expect(saved.providers[0]).toMatchObject({ preset: "cloudflare-workers-ai", base_url: base,
    explicit_credentials_only: true, credentials: [], supported_models: [], test_policy: { automaticEnabled: false, models: [] },
    call_flow: { call_adapter: "auto", fingerprint_adapter: "auto", special_adapter: "auto" } });
  expect(saved.providers[0]).not.toHaveProperty("site_health");
  expect(saved.model_routes).toEqual([]);
  expect(templateAdaptationComplete("cloudflare-workers-ai")).toBe(false);
});

it("imports the account-bound official directory without generation or cross-account evidence reuse", async () => {
  const discover = vi.fn().mockResolvedValue(catalog); const generic = vi.fn(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "cf", label: "Cloudflare" }]} existingCredentialIds={[]}
    locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit}
    catalogProviderKind={() => catalogProviderKind(provider)} onDiscoverProviderModels={discover} onDiscover={generic} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-cf-token" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(discover).toHaveBeenCalledWith({ providerId: "cf", apiKey: "synthetic-cf-token" }, expect.any(AbortSignal));
  expect(generic).not.toHaveBeenCalled();
  const doc = { providers: [provider], model_routes: [], aliases: {} };
  const saved = applyOfficialCatalog(doc, "cf", "one", catalog);
  expect(saved.providers[0]).toMatchObject({ credentials: [{ model_selection: { fetched: catalog.models, manual: ["@cf/manual/exact"] } }] });
  expect(JSON.stringify(saved)).not.toContain("verified_models");
  expect(applyOfficialCatalog(saved, "cf", "one", catalog)).toEqual(saved);
  expect(() => applyOfficialCatalog(doc, "cf", "one", { ...catalog, accountBaseUrl: cloudflareBase("a".repeat(32))! })).toThrow();
});

it("rejects non-account URLs and template placeholders rather than sending a token to them", () => {
  for (const id of ["", "trial", "a".repeat(31), "g".repeat(32), "${CLOUDFLARE_ACCOUNT_ID}"]) expect(cloudflareBase(id)).toBeNull();
  for (const value of [base.replace("https:", "http:"), `${base}?x=1`, `${base}#fragment`, base.replace("api.cloudflare.com", "api.cloudflare.com.evil.invalid"), base.replace(account, `${account}/../other`)]) {
    expect(cloudflareEndpoint(value)).toBeNull();
  }
});
