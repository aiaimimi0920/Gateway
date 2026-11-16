import "@testing-library/jest-dom/vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { ProviderSessionMaterialField } from "./ProviderSessionMaterialField";
import { parseProviderSessionMaterial } from "./providerSessionMaterial";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";
import type { SessionProduct } from "./providerSessionProducts";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";

const product = "perplexity-web" as SessionProduct;
const source = JSON.stringify({ product, token: "synthetic-session", userAgent: " original-UA ", expiresAt: "2099-01-01T00:00:00Z" });
function staged() {
  const active = routeDocument([{ id: "renamed", preset: product, base_url: "https://www.perplexity.ai", explicit_credentials_only: true, credentials: [{ id: "exact" }] }]);
  const material = parseProviderSessionMaterial(source, product);
  return { active, ...applyProviderSessionMaterial(active, "renamed", "exact", material) };
}
function patches(result: ReturnType<typeof staged>) {
  return buildCredentialSecretPatches({ activeDocument: result.active, draftDocument: result.document, activeSecrets: [], credentialSecretEdits: result.secretEdits });
}
it("adds a distinct empty Web pool beside the untouched Sonar template", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === product)!;
  expect(template).toMatchObject({ preset: product, baseUrl: "https://www.perplexity.ai", supportedModels: [] });
  const workflow = defaultCatalogWorkflow(template);
  const document = addCatalogProvider(routeDocument([]), template, { templateId: product, providerId: "renamed", providerLabel: "renamed", vendorKey: "perplexity", vendorName: "Perplexity", baseUrl: template.baseUrl, supportedModels: [], workflow });
  expect(document.providers[0]).toMatchObject({ explicit_credentials_only: true, credentials: [], call_flow: { special_adapter: "perplexity_web", fingerprint_adapter: "session" } });
  expect(document.model_routes).toEqual([]);
  expect(PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "perplexity-api")).toMatchObject({ preset: "perplexity-api", baseUrl: "https://api.perplexity.ai" });
});
it("normal material field requires the original UA and submits only normalized product material", () => {
  const onMaterial = vi.fn();
  render(<ProviderSessionMaterialField product={product} disabled={false} loaded={false} resetKey="exact" onMaterial={onMaterial} onClear={vi.fn()} />);
  const field = screen.getByLabelText("会话材料 JSON");
  expect(field.getAttribute("placeholder")).toContain("userAgent");
  fireEvent.change(field, { target: { value: JSON.stringify({ product, token: "synthetic-session" }) } });
  fireEvent.click(screen.getByText("读取会话材料"));
  expect(onMaterial).not.toHaveBeenCalled();
  fireEvent.change(field, { target: { value: source } });
  fireEvent.click(screen.getByText("读取会话材料"));
  expect(onMaterial).toHaveBeenCalledWith(expect.objectContaining({ product, userAgent: " original-UA ", cookieHeader: "__Secure-next-auth.session-token=synthetic-session" }));
  expect(field).toHaveValue("");
});
it("keeps session and Cookie only in normal secret patches, preserving UA and earliest expiry", () => {
  const result = staged();
  expect(JSON.stringify(result.document)).not.toContain("synthetic-session");
  expect(result.document.providers[0]).toMatchObject({ credentials: [{ headers: { "User-Agent": " original-UA " }, expires_at: "2099-01-01T00:00:00.000Z", session_auth: { transport: "cookie", primary_cookie_name: "__Secure-next-auth.session-token", expires_at: "2099-01-01T00:00:00.000Z" } }] });
  expect(patches(result)).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-session" });
});
it("carries the earliest storage-state Cookie expiry through normal apply and commit validation", () => {
  const cookie = (name: string, value: string, year: number) => ({ name, value, domain: ".perplexity.ai", path: "/", secure: true, expires: Date.parse(`${year}-01-01T00:00:00Z`) / 1000 });
  const material = parseProviderSessionMaterial(JSON.stringify({ product, userAgent: "original-UA", expiresAt: "2099-01-01T00:00:00Z", storageState: { cookies: [
    cookie("__Secure-next-auth.session-token", "synthetic-session", 2090), cookie("cf_clearance", "synthetic-clearance", 2080),
  ] } }), product);
  const { active } = staged();
  const result = { active, ...applyProviderSessionMaterial(active, "renamed", "exact", material) };
  expect(result.document.providers[0]).toMatchObject({ credentials: [{ expires_at: "2080-01-01T00:00:00.000Z", session_auth: { expires_at: "2080-01-01T00:00:00.000Z" } }] });
  expect(patches(result)).toContainEqual({ path: "/providers/0/credentials/0/headers/Cookie", operation: "replace", value: "__Secure-next-auth.session-token=synthetic-session; cf_clearance=synthetic-clearance" });
});
it.each(["changed", "deleted", "duplicate", "inherited", "expiry", "api-pool"])("rejects pending Perplexity material mutation: %s", (mutation) => {
  const result = staged();
  const provider = result.document.providers[0] as Record<string, any>;
  const credential = provider.credentials[0];
  if (mutation === "changed") credential.headers["User-Agent"] = "other";
  if (mutation === "deleted") delete credential.headers["User-Agent"];
  if (mutation === "duplicate") credential.headers["user-agent"] = " original-UA ";
  if (mutation === "inherited") { delete credential.headers["User-Agent"]; provider.headers = { "User-Agent": " original-UA " }; }
  if (mutation === "expiry") credential.expires_at = credential.session_auth.expires_at = "2099-02-01T00:00:00Z";
  if (mutation === "api-pool") provider.preset = "perplexity-api";
  expect(() => patches(result)).toThrow();
});
