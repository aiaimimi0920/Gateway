import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { PROVIDER_CATALOG_TEMPLATES, type ProviderCatalogDraft } from "./providerCatalog";
import { defaultCatalogWorkflow, catalogWorkflowError, catalogHealthAdapter, compatibleCatalogDrivers } from "./providerCatalogWorkflow";
import { addCatalogProvider } from "./providerCatalogCreation";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { captureProviderCreationSecrets, providerCreationSecretPatches } from "./providerCreationSecrets";

const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.custom)!;
function value(): ProviderCatalogDraft {
  return { templateId: template.id, providerId: "fixture", providerLabel: "fixture", vendorKey: "fixture", vendorName: "fixture",
    baseUrl: "https://fixture.test/v1", supportedModels: [], workflow: defaultCatalogWorkflow(template), secrets: {} };
}
const empty = (): ConsoleRouteDocument => ({ providers: [], model_routes: [], aliases: {} });

describe("provider-only creation", () => {
  it("preserves an empty visible pool without a default account or test target", () => {
    const original = empty(); const next = addCatalogProvider(original, template, value());
    expect(original.providers).toEqual([]);
    const catalog = buildRouteAccountCatalog(next);
    expect(catalog.accounts).toEqual([]);
    expect(catalog.providerBuckets.flatMap((bucket) => bucket.providers)).toHaveLength(1);
    expect(testProviderContext(next, "fixture")?.accounts).toEqual([]);
    expect(catalog.groups.find((group) => group.id === "default")?.providerCredentialIds).toEqual([]);
    expect(next.providers[0]).toMatchObject({ explicit_credentials_only: true, credentials: [] });
  });
  it("keeps legacy implicit accounts unchanged", () => {
    const document = empty(); document.providers.push({ id: "legacy", base_url: "https://fixture.test" });
    expect(buildRouteAccountCatalog(document).accounts[0].id).toBe("legacy::default");
  });
  it("writes actual adapter, fingerprint, mappings and policies, never UI-only protocol fields", () => {
    const draft = value(); draft.requestClientProfile = "claude_cli";
    draft.workflow!.callAdapter = "anthropic";
    draft.workflow!.mappings = { requested: ["upstream-a", "upstream-b"], unlinked: [] };
    const next = addCatalogProvider(empty(), template, draft);
    expect(next.providers[0]).toMatchObject({ adapter: "anthropic_compatible", protocol_profile: "anthropic",
      request_client_profile: "claude_cli", model_map_targets: { requested: ["upstream-a", "upstream-b"] },
      call_flow: { call_adapter: "anthropic", fingerprint_adapter: "claude_cli", special_adapter: "none" },
      pool_min_size: 1, pool_target_size: 100, test_policy: { automaticEnabled: false } });
    expect(next.model_routes).toEqual([{ pattern: "requested", provider_ids: ["fixture"] }]);
    expect(JSON.stringify(next)).not.toContain("protocolPreference");
    expect((next.providers[0] as Record<string, unknown>).model_map_targets).not.toHaveProperty("unlinked");
  });
  it.each(["gemini_web", "chatgpt_web", "aistudio_web"] as const)("selects existing specialized preset %s without advertising acceptance", (specialAdapter) => {
    const draft = value(); draft.workflow!.specialAdapter = specialAdapter; draft.requestClientProfile = "codex_cli";
    const provider = addCatalogProvider(empty(), template, draft).providers[0];
    expect(provider).toHaveProperty("preset"); expect(provider).not.toHaveProperty("request_client_profile");
    expect(provider).not.toHaveProperty("adapter");
    expect(provider).toHaveProperty("call_flow", { call_adapter: "auto", fingerprint_adapter: "session", special_adapter: specialAdapter });
  });
  it("matches health products rather than all services under a brand", () => {
    const health = (id: string) => catalogHealthAdapter(PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === id)!);
    expect(health("chatgpt")).toBe("openai-status"); expect(health("openai")).toBeNull();
    expect(health("gemini-web")).toBeNull(); expect(health("gemini-api")).toBe("gemini-status");
  });
  it("only offers exact backend-allowlisted drivers", () => {
    const drivers = [{ id: "first", mode: "script" as const, providerIds: ["fixture"] }, { id: "other", mode: "http" as const, providerIds: ["another"] }];
    expect(compatibleCatalogDrivers(drivers, "fixture").map((driver) => driver.id)).toEqual(["first"]);
    expect(compatibleCatalogDrivers(drivers, "fixture-2")).toEqual([]);
  });
  it("validates bounds, test cases and health authentication", () => {
    const workflow = defaultCatalogWorkflow(template); expect(catalogWorkflowError(workflow)).toBeNull();
    workflow.poolMin = 101; expect(catalogWorkflowError(workflow)).not.toBeNull(); workflow.poolMin = 1;
    workflow.health.auth.kind = "bearer"; expect(catalogWorkflowError(workflow)).toContain("自动识别不会广播密钥");
  });
  it("requires a WebDAV username only when a password is supplied", () => {
    const workflow = defaultCatalogWorkflow(template);
    workflow.storage = { type: "webdav", endpoint: "https://storage.test", username: "" };
    expect(catalogWorkflowError(workflow)).toBeNull();
    expect(catalogWorkflowError(workflow, { "credential_storage_connection/password": "fixture" })).toContain("用户名");
    workflow.storage.username = "fixture";
    expect(catalogWorkflowError(workflow, { "credential_storage_connection/password": "fixture" })).toBeNull();
  });
});

describe("creation secret ownership", () => {
  function fixture() {
    const draft = value(); draft.workflow!.storage = { type: "webdav", endpoint: "https://storage.test", username: "fixture", directory: "pool" };
    draft.workflow!.health.enabled = true; draft.workflow!.health.adapter = "new-api"; draft.workflow!.health.auth.kind = "bearer";
    draft.secrets = { "credential_storage_connection/password": "storage-secret", "site_health/auth/token": "health-secret" };
    const document = addCatalogProvider(empty(), template, draft);
    return { draft, document, edits: captureProviderCreationSecrets(document, draft.providerId, draft.secrets) };
  }
  it("keeps secrets out of ordinary route JSON and resolves reordered slots at commit", () => {
    const { document, edits } = fixture(); expect(JSON.stringify(document)).not.toContain("secret");
    document.providers.unshift({ id: "another" });
    expect(providerCreationSecretPatches(document, edits)).toEqual([
      { path: "/providers/1/credential_storage_connection/password", operation: "replace", value: "storage-secret" },
      { path: "/providers/1/site_health/auth/token", operation: "replace", value: "health-secret" },
    ]);
  });
  it("rejects changed authentication destinations and ignores deleted owners", () => {
    const { document, edits } = fixture();
    (document.providers[0] as Record<string, unknown>).credential_storage_connection = { type: "webdav", endpoint: "https://other.test", username: "fixture" };
    expect(() => providerCreationSecretPatches(document, edits)).toThrow("目标已变化");
    document.providers = []; expect(providerCreationSecretPatches(document, edits)).toEqual([]);
  });
});
