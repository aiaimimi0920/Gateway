import { expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { editQwenProviderEndpoint } from "./qwenProviderEndpoint";
import { QWEN_REGIONS, buildQwenEndpoint } from "./qwenEndpoint";
import { qwenMaterialTarget } from "./qwenMaterialBinding";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";

const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const other = base.replace("workspace-a", "account-b");
const catalog = { sourceUrl: base.replace("/compatible-mode/v1", "/api/v1/models"), accountBaseUrl: base,
  checkedAt: "2026-11-02T00:00:00Z", models: ["fetched"] };
const active = (): ConsoleRouteDocument => ({ providers: [{ id: "renamed", preset: "qwen-official-api", explicit_credentials_only: true, base_url: base,
  credentials: [{ id: "a", enabled: false, model_catalog: catalog, supported_models: ["manual", "fetched"] }, { id: "b", base_url: other }] },
  { id: "coding", preset: "qwen-coding-plan-openai" }], model_routes: [{ pattern: "fetched", enabled: false, priority: 42 }], aliases: { same: "manual" } });
const change = (doc: ConsoleRouteDocument, preserve = true, region = "eu-central-1") => editQwenProviderEndpoint(doc, "renamed", JSON.stringify(doc.providers[0]),
  { workspaceId: "new-workspace", region }, preserve);
it.each(QWEN_REGIONS)("edits %s default without migrating existing accounts or evidence", (region) => {
  const doc = active(); const saved = structuredClone(doc); const next = change(doc, true, region);
  expect(doc).toEqual(saved);
  expect(next.providers[0]).toMatchObject({ base_url: buildQwenEndpoint({ workspaceId: "new-workspace", region }),
    credentials: [{ id: "a", enabled: false, base_url: base, model_catalog: catalog, supported_models: ["manual", "fetched"] }, { id: "b", base_url: other }] });
  expect(next.model_routes).toEqual(doc.model_routes); expect(next.aliases).toEqual(doc.aliases); expect(next.providers[1]).toEqual(doc.providers[1]);
  const value = credentialDialogValueFromDocument(next, "renamed", "a")!;
  expect(value.modelCatalog).toEqual(catalog); expect(value.fetchedModelsText).toBe("fetched"); expect(value.manualModelsText).toBe("manual");
});
it("needs explicit consent for inherited accounts, not existing overrides or an empty pool", () => {
  const doc = active(); expect(() => change(doc, false)).toThrow("固定原目标");
  (doc.providers[0] as Record<string, unknown>).credentials = [{ id: "override", base_url: other }];
  expect(change(doc, false).providers[0]).toMatchObject({ credentials: [{ id: "override", base_url: other }] });
  (doc.providers[0] as Record<string, unknown>).credentials = [];
  expect(change(doc, false).providers[0]).toMatchObject({ credentials: [] });
});
it.each([{ credentials: [] }, { credentials: [{ id: "override", base_url: other }] }])("initializes an empty parent only when no account inherits an unknown audience", ({ credentials }) => {
  const doc = active(); Object.assign(doc.providers[0] as Record<string, unknown>, { base_url: "", credentials });
  expect(change(doc, false).providers[0]).toMatchObject({ base_url: expect.stringContaining("new-workspace"), credentials });
  Object.assign(doc.providers[0] as Record<string, unknown>, { credentials: [{ id: "unknown" }] });
  expect(() => change(doc)).toThrow("工作空间");
});
it("does not rewrite staged bindings; the final commit builder accepts the unchanged effective audience", () => {
  const doc = active(); const edit: CredentialSecretEdit = { providerId: "renamed", credentialId: "a", field: "api_key", operation: "replace",
    value: "synthetic-pending-key", qwenBinding: qwenMaterialTarget(doc, "renamed", "a") };
  const original = structuredClone(edit);
  const next = change(doc);
  expect(buildCredentialSecretPatches({ activeDocument: doc, draftDocument: next, activeSecrets: [], credentialSecretEdits: [edit] }))
    .toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-pending-key" }]);
  expect(edit).toEqual(original); expect(qwenMaterialTarget(next, "renamed", "a")).toEqual(edit.qwenBinding);
  ((next.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0].base_url = other;
  expect(() => buildCredentialSecretPatches({ activeDocument: doc, draftDocument: next, activeSecrets: [], credentialSecretEdits: [edit] })).toThrow("目标已变化");
});
it("rejects stale drafts, malformed identity/targets, parent materials and non-explicit products atomically", () => {
  const doc = active(); const expected = JSON.stringify(doc.providers[0]);
  (doc.providers[0] as Record<string, unknown>).label = "changed";
  expect(() => editQwenProviderEndpoint(doc, "renamed", expected, { region: "cn-beijing", workspaceId: "new" }, true)).toThrow("草稿已变化");
  for (const patch of [{ preset: "qwen-coding-plan-openai" }, { explicit_credentials_only: false }, { api_key: "synthetic-parent" },
    { base_url: "https://evil.invalid" }, { credentials: [{ id: "a", base_url: "" }] }, { credentials: [{ id: "a" }, { id: "a" }] }, { credentials: {} }]) {
    const bad = active(); Object.assign(bad.providers[0] as Record<string, unknown>, patch); const before = structuredClone(bad);
    expect(() => change(bad)).toThrow(); expect(bad).toEqual(before);
  }
  expect(() => change(active(), true, "unknown-region")).toThrow("地域");
});
