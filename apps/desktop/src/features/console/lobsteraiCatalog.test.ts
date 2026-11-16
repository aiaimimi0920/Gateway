import { expect, it } from "vitest";
import { LOBSTERAI_TEMPLATE, isLobsterAiProvider } from "./lobsteraiCatalog";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";
import { providerAccessKind } from "./providerAccessKind";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { savedLobsterAiBinding, stageLobsterAiCatalog, lobsteraiCatalogFields, lobsteraiModelCatalogSchema } from "./lobsteraiModelCatalog";
import { lobsteraiAuthorizationUrl } from "./lobsteraiAuthorizationUrl";
import { authorization, catalog, model, provider } from "./lobsteraiTestFixtures";
it("creates a visible empty subscription pool without guessed models or automatic calls", () => {
  const t = LOBSTERAI_TEMPLATE; expect(PROVIDER_CATALOG_TEMPLATES).toContain(t);
  const document = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, t, { templateId: t.id, providerId: "renamed", providerLabel: "Own account", vendorKey: t.vendorKey, vendorName: t.vendorName, baseUrl: t.baseUrl, supportedModels: [], workflow: defaultCatalogWorkflow(t) });
  expect(document.providers[0]).toMatchObject({ auth_mode: "lobsterai_session", adapter: "openai_compatible", explicit_credentials_only: true, credentials: [], test_policy: { automaticEnabled: false } });
  expect(document.providers[0]).not.toHaveProperty("preset"); expect(document.model_routes).toEqual([]);
  expect(isLobsterAiProvider(document.providers[0] as Record<string, unknown>)).toBe(true);
  expect(providerAccessKind({ authMode: "lobsterai_session", providerId: "renamed" })).toBe("oauth");
});
it("stages account directory and preserves manual models and unknown capabilities", () => {
  const doc = { providers: [provider], model_routes: [], aliases: {} }; const value = credentialDialogValueFromDocument(doc, "renamed", "stable")!;
  expect(value.fetchedModelsText).toBe("old-fetched"); expect(value.manualModelsText).toBe("manual-model");
  const next = stageLobsterAiCatalog(value, catalog, savedLobsterAiBinding(provider, "stable")!);
  expect(next.fetchedModelsText).toBe("opaque/model"); expect(next.manualModelsText).toBe("manual-model");
  expect(lobsteraiCatalogFields(provider, next)).toEqual({ lobsterai_catalog: catalog });
  expect(next.lobsteraiCatalog?.models[0].supportsToolCalling).toBeNull();
  expect(() => stageLobsterAiCatalog({ ...value, credentialId: "other" }, catalog, savedLobsterAiBinding(provider, "stable")!)).toThrow();
  expect(() => stageLobsterAiCatalog(value, { models: [model("a,b")] }, savedLobsterAiBinding(provider, "stable")!)).toThrow();
});
it.each([{ models: [model(), model()] }, { models: [{ ...model(), contextWindow: 0 }] }, { models: [{ ...model(), accessible: false }] }])("rejects invalid declarations %j", (value) => { expect(lobsteraiModelCatalogSchema.safeParse(value).success).toBe(false); });
it("permits only the official fragment route with an exact loopback callback", () => {
  expect(lobsteraiAuthorizationUrl(authorization)).toBe(authorization);
  for (const [from, to] of [["/portal", "/redirect"], ["#/login?", "#/other?"], ["source=electron", "source=electron&state=duplicate"], ["youdao.com", "youdao.com.evil.invalid"], ["https://", "http://"], ["lobsterai.youdao.com", "user@lobsterai.youdao.com"], ["127.0.0.1", "evil.invalid"], ["lobsterai.youdao.com", "lobsterai.youdao.com:444"]]) expect(lobsteraiAuthorizationUrl(authorization.replace(from, to))).toBeNull();
});
