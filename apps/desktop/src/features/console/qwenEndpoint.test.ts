import { describe, expect, it } from "vitest";
import { buildQwenEndpoint, emptyQwenEndpoint, parseQwenEndpoint, QWEN_REGIONS, requireQwenEndpoint } from "./qwenEndpoint";
import { clearQwenEvidence, normalizeQwenCredentialDraft } from "./qwenCredentialDraft";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { duplicateCredentialDialogValue } from "./providerCredentialDraft";
import type { AccountsLedgerPilotAccount } from "./AccountsLedgerWorkspace";
import type { CredentialDialogValue } from "./CredentialDialog";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const provider = { id: "renamed", preset: "qwen-official-api", base_url: base };
const value = (): CredentialDialogValue => ({ providerId: "renamed", credentialId: "a", accountName: "", enabled: true,
  baseUrl: "", supportedModelsText: "old-fetched\nmanual", fetchedModelsText: "old-fetched", manualModelsText: "manual",
  apiKeyOperation: "keep", apiKeyValue: "" });

describe("ordinary Qwen public endpoint", () => {
  it.each(QWEN_REGIONS)("roundtrips only the explicit %s workspace target", (region) => {
    const target = { region, workspaceId: "workspace-a" };
    const endpoint = buildQwenEndpoint(target)!;
    expect(parseQwenEndpoint(endpoint)).toEqual(target);
    expect(requireQwenEndpoint(`${endpoint}/`)).toBe(endpoint);
  });
  it.each(["", "trial", "Trial", "-a", "a-", "a.b", "a_b", "a/b", "a%2fb", "a b", "a\nb", "a".repeat(64)])(
    "rejects invalid or reserved workspace %j without choosing a fallback", (workspaceId) => {
      expect(buildQwenEndpoint({ region: "cn-beijing", workspaceId })).toBeNull();
    });
  it("has no default region and rejects foreign products, ports and ambiguous URL forms", () => {
    expect(emptyQwenEndpoint()).toEqual({ region: "", workspaceId: "" });
    expect(buildQwenEndpoint({ region: "", workspaceId: "a" })).toBeNull();
    expect(buildQwenEndpoint({ region: "us-west-2", workspaceId: "a" })).toBeNull();
    for (const address of [base.replace("https:", "http:"), base.replace("https://", "https://user@"),
      base.replace(".com/", ".com:443/"), base.replace("workspace-a", "WORKSPACE-A"), `${base}?q=1`, `${base}#f`,
      `${base}//`, base.replace("/compatible-mode/v1", "/compatible-mode/v1/models"),
      "https://coding.dashscope.aliyuncs.com/v1", "https://chat.qwen.ai", "https://example.invalid/v1"]) {
      expect(parseQwenEndpoint(address), address).toBeNull();
      expect(() => requireQwenEndpoint(address)).toThrow();
    }
  });
  it("revalidates pure creation and does not create an account, model, route or secret", () => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "qwen-api")!;
    const draft: ProviderCatalogDraft = { templateId: template.id, providerId: "renamed", providerLabel: "", vendorKey: "qwen",
      vendorName: "Qwen", baseUrl: base, supportedModels: [], workflow: defaultCatalogWorkflow(template) };
    const active = routeDocument([]);
    const next = addCatalogProvider(active, template, draft);
    expect(active.providers).toEqual([]);
    expect(next.providers[0]).toMatchObject({ preset: "qwen-official-api", base_url: base, explicit_credentials_only: true, credentials: [] });
    expect(next.model_routes).toEqual([]);
    expect(buildRouteAccountCatalog(next).accounts).toEqual([]);
    expect(JSON.stringify(next)).not.toMatch(/api_key|discovery_job|refresh_token/);
    expect(() => addCatalogProvider(active, template, { ...draft, baseUrl: "https://coding.dashscope.aliyuncs.com/v1" })).toThrow();
    expect(() => addCatalogProvider(active, template, { ...draft, qwenEndpoint: { region: "eu-central-1", workspaceId: "workspace-a" } })).toThrow();
  });
});

describe("Qwen target and model evidence", () => {
  it("compares effective targets, not inherit/override representation", () => {
    expect(normalizeQwenCredentialDraft(provider, { ...value(), baseUrl: `${base}/` }, value()).apiKeyOperation).toBe("keep");
    expect(normalizeQwenCredentialDraft(provider, value(), { ...value(), baseUrl: base }).baseUrl).toBe("");
    const changed = { ...value(), baseUrl: base.replace("cn-beijing", "eu-central-1") };
    expect(() => normalizeQwenCredentialDraft(provider, changed, value())).toThrow("目标已变化");
    expect(() => normalizeQwenCredentialDraft(provider, { ...value(), qwenEndpoint: emptyQwenEndpoint() }, value())).toThrow();
  });
  it.each(["replace", "clear"] as const)("%s preserves manual ownership but revokes old fetched evidence", (apiKeyOperation) => {
    const next = normalizeQwenCredentialDraft(provider, { ...value(), baseUrl: base.replace("cn-beijing", "eu-central-1"), apiKeyOperation }, value());
    expect(next).toMatchObject({ fetchedModelsText: "", manualModelsText: "manual", supportedModelsText: "manual" });
    expect(next.modelCatalog).toBeUndefined(); expect(next.discovery).toBeUndefined();
  });
  it("clears fetched values rather than washing them into manual selections", () => {
    expect(clearQwenEvidence(value())).toMatchObject({ fetchedModelsText: "", manualModelsText: "manual", supportedModelsText: "manual" });
    const generic = { ...provider, preset: "qwen-coding-plan" };
    expect(normalizeQwenCredentialDraft(generic, value())).toEqual(value());
  });
  it.each(["replace", "clear"] as const)("allows explicit %s to repair an invalid old target, never keep", (apiKeyOperation) => {
    const previous = { ...value(), baseUrl: "https://invalid.example" };
    expect(() => normalizeQwenCredentialDraft(provider, value(), previous)).toThrow();
    expect(normalizeQwenCredentialDraft(provider, { ...value(), baseUrl: base, apiKeyOperation }, previous))
      .toMatchObject({ baseUrl: base, fetchedModelsText: "", manualModelsText: "manual" });
  });
  it("duplicates only public targets and manual choices, never old Key or directory ownership", () => {
    const document = routeDocument([{ ...provider, credentials: [{ id: "a", base_url: base, api_key: "synthetic-not-to-copy",
      supported_models: ["old-fetched", "manual"], model_selection: { fetched: ["old-fetched"], manual: ["manual"] } }] }]);
    const copy = duplicateCredentialDialogValue(document, "renamed", { accountId: "a", enabled: true } as AccountsLedgerPilotAccount);
    expect(copy).toMatchObject({ baseUrl: base, apiKeyOperation: "replace", apiKeyValue: "", supportedModelsText: "manual", manualModelsText: "manual", fetchedModelsText: "" });
    expect(JSON.stringify(copy)).not.toContain("synthetic-not-to-copy");
  });
});
