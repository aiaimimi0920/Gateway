// The compatibility template takes an explicit profile, never a guessed region, scope or entitlement.
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";

const profile = "arn:aws:codewhisperer:eu-central-1:123456789012:profile/synthetic";
const baseUrl = "https://runtime.eu-central-1.kiro.dev";
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
function template() {
  const value = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "kiro-oauth");
  expect(value).toBeDefined(); return value!;
}
it("creates an empty OAuth runtime pool bound to an explicit profile without Key or OAuth material", () => {
  const entry = template();
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, {
    templateId: entry.id, providerId: "kiro-oauth", providerLabel: "kiro-oauth",
    vendorKey: entry.vendorKey, vendorName: entry.vendorName, baseUrl, supportedModels: [],
    kiroProfileArn: profile, workflow: defaultCatalogWorkflow(entry),
  });
  expect(result.providers[0]).toMatchObject({ preset: "kiro", auth_mode: "kiro_oauth_runtime", base_url: baseUrl,
    responses_path: "/", extra_body: { profileArn: profile }, explicit_credentials_only: true, credentials: [],
    supported_models: [], test_policy: { automaticEnabled: false, models: [] },
    call_flow: { call_adapter: "auto", special_adapter: "kiro", fingerprint_adapter: "session" } });
  for (const key of ["api_key", "kiro_oauth", "refresh_token", "default_model"]) expect(result.providers[0]).not.toHaveProperty(key);
  expect(result.model_routes).toEqual([]);
  expect(result.aliases).toEqual({});
});
it("requires an explicit profile before adding, derives its business region and requests no secret access", () => {
  const submit = vi.fn(); const secret = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={["kiro-oauth", "kiro-oauth-2"]}
    existingCredentialIds={[]} locked={false} hasSecretAccess={false} onOpenChange={vi.fn()}
    onRequestSecretAccess={secret} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^kiro-oauth/ }));
  expect(screen.getByLabelText("Base URL")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Profile ARN"), { target: { value: profile } });
  expect(screen.getByLabelText("Base URL")).toHaveValue(baseUrl);
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "kiro-oauth", providerId: "kiro-oauth-3", baseUrl, kiroProfileArn: profile }));
  expect(secret).not.toHaveBeenCalled();
});
it("rejects a mismatch or unsupported profile without substituting the directory or US East", () => {
  const entry = template();
  for (const [arn, base] of [[profile, "https://runtime.us-east-1.kiro.dev"],
    [profile.replace("eu-central-1", "eu-west-1"), "https://runtime.eu-west-1.kiro.dev"],
    ["", "https://runtime.us-east-1.kiro.dev"], [profile.replace("profile/", "application/"), baseUrl]]) {
    expect(() => addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry, {
      templateId: entry.id, providerId: "pool", providerLabel: "pool", vendorKey: entry.vendorKey, vendorName: entry.vendorName,
      baseUrl: base, supportedModels: [], kiroProfileArn: arn,
    })).toThrow();
  }
});
