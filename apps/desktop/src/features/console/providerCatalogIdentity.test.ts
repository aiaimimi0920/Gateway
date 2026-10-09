import { describe, expect, it } from "vitest";
import { availableCatalogId } from "./providerCatalogIdentity";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";
import { addProviderWithCredential } from "./credentialDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";

describe("provider template instances", () => {
  it("allocates every template repeatedly without a type-level singleton", () => {
    for (const template of PROVIDER_CATALOG_TEMPLATES) {
      const ids: string[] = [];
      for (let count = 0; count < 4; count += 1) {
        const id = availableCatalogId(template.providerId, ids);
        expect(ids).not.toContain(id);
        ids.push(id);
      }
      expect(ids).toEqual([template.providerId, ...[2, 3, 4].map((n) => `${template.providerId}-${n}`)]);
    }
  });

  it("normalizes occupied IDs and checks non-consecutive suffix collisions", () => {
    expect(availableCatalogId("pool", [" pool ", "pool-2", "pool-4"])).toBe("pool-3");
    expect(availableCatalogId("new", ["pool"])).toBe("new");
  });

  it.each(["openai", "custom-api-provider"])("preserves independent %s instances, accounts and shared routes", (templateId) => {
    const template = PROVIDER_CATALOG_TEMPLATES.find((item) => item.id === templateId)!;
    let document: ConsoleRouteDocument = { providers: [], model_routes: [], aliases: {}, account_groups: [] };
    for (const suffix of ["", "-2"]) {
      const id = `${template.providerId}${suffix}`;
      const provider = providerDefinitionFromCatalogDraft(template, {
        templateId, providerId: id, providerLabel: id, vendorKey: template.vendorKey,
        vendorName: template.vendorName, baseUrl: `https://pool${suffix || "-1"}.example.test`,
        supportedModels: ["shared-model"], credentialId: `${id}-account-1`, accountName: id, apiKey: "fixture",
      });
      document = addProviderWithCredential(document, { provider, credential: { id: `${id}-account-1` }, routePatterns: ["shared-model"] });
    }
    expect(document.providers).toHaveLength(2);
    expect(document.providers).toMatchObject([
      { vendor_key: template.vendorKey, base_url: "https://pool-1.example.test", credentials: [{ id: `${template.providerId}-account-1` }] },
      { vendor_key: template.vendorKey, base_url: "https://pool-2.example.test", credentials: [{ id: `${template.providerId}-2-account-1` }] },
    ]);
    expect(document.model_routes).toEqual([{ pattern: "shared-model", provider_ids: [template.providerId, `${template.providerId}-2`] }]);
  });
});
