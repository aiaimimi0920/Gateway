import { describe, expect, it } from "vitest";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { dimCatalogFields, dimCatalogResponseSchema, savedDimBinding, stageDimCatalog } from "./dimModelCatalog";

import { catalog, document, model } from "./dimCatalogTestFixtures";
describe("DIM declared directory ownership", () => {
  it("keeps declarations distinct from verified calls and classifies legacy fetched models", () => {
    const value = credentialDialogValueFromDocument(document, "dim", "account")!;
    expect(value.fetchedModelsText).toBe("old-fetched"); expect(value.manualModelsText).toBe("manual-model");
    const next = stageDimCatalog(value, catalog, savedDimBinding(document.providers[0], "account")!);
    expect(next.fetchedModelsText).toBe("opaque-model"); expect(next.manualModelsText).toBe("manual-model");
    expect(next.supportedModelsText).toBe("manual-model\nopaque-model");
    expect(dimCatalogFields(document.providers[0], next)).toEqual({ dim_catalog: catalog });
    expect(JSON.stringify(next)).not.toContain("verified_models");
  });
  it.each(["site", "client_id", "subject"] as const)("rejects a directory from a different %s", (field) => {
    const value = credentialDialogValueFromDocument(document, "dim", "account")!;
    expect(() => stageDimCatalog(value, { ...catalog, [field]: "foreign" }, savedDimBinding(document.providers[0], "account")!)).toThrow();
  });
  it("does not borrow saved identity for replacement, renamed accounts or target changes", () => {
    const value = credentialDialogValueFromDocument(document, "dim", "account")!;
    for (const patch of [{ apiKeyOperation: "replace" as const }, { credentialId: "other" }, { baseUrl: "https://dimagent.com/v1" }]) {
      expect(() => stageDimCatalog({ ...value, ...patch }, catalog, savedDimBinding(document.providers[0], "account")!)).toThrow();
    }
    expect(savedDimBinding({ ...document.providers[0], base_url: "https://foreign.invalid" }, "account")).toBeNull();
  });
  it("supports an explicit empty directory without erasing manual models", () => {
    const value = credentialDialogValueFromDocument(document, "dim", "account")!;
    const next = stageDimCatalog(value, { ...catalog, models: [] }, savedDimBinding(document.providers[0], "account")!);
    expect(next.supportedModelsText).toBe("manual-model"); expect(next.fetchedModelsText).toBe("");
  });
  it("does not silently split or trim opaque model IDs when staging the shared text editor", () => {
    const value = credentialDialogValueFromDocument(document, "dim", "account")!;
    for (const id of ["has,comma", " padded"]) {
      expect(() => stageDimCatalog(value, { ...catalog, models: [model(id)] }, savedDimBinding(document.providers[0], "account")!)).toThrow(/无损/);
    }
  });
  it("rejects duplicate or secret-bearing wire data rather than accepting partial metadata", () => {
    const response = { revision: "r1", providerId: "dim", credentialId: "account", catalog };
    expect(dimCatalogResponseSchema.safeParse(response).success).toBe(true);
    for (const invalid of [{ ...catalog, access_token: "secret" }, { ...catalog, models: [model(), model()] },
      { ...catalog, models: [{ ...model(), protocol: "unknown" }] }, { ...catalog, models: [model("bad\nmodel")] }]) {
      expect(dimCatalogResponseSchema.safeParse({ ...response, catalog: invalid }).success).toBe(false);
    }
  });
});
