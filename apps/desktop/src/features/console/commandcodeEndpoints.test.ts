import { describe, expect, it } from "vitest";
import { applyOfficialCatalog, catalogSources, officialCatalogSchema } from "./officialModelCatalog";
import type { ConsoleRouteDocument } from "../../api/contracts";

const catalog = {
  sourceUrl: catalogSources.commandcode,
  checkedAt: "2026-11-13T00:00:00Z",
  models: ["declared-model"],
  supportedEndpoints: { "declared-model": ["/chat/completions", "/responses"] },
};

describe("Command Code endpoint declarations", () => {
  it.each(["/messages", "/responses"])("keeps a native-only %s model in normal account saves", (endpoint) => {
    const native = { ...catalog, supportedEndpoints: { "declared-model": [endpoint] } };
    const document = {
      providers: [{ id: "cmd", preset: "commandcode-api", base_url: "https://api.commandcode.ai/provider/v1",
        credentials: [{ id: "account", supported_models: [] }], supported_models: [] }],
      model_routes: [],
    } as unknown as ConsoleRouteDocument;
    const saved = JSON.parse(JSON.stringify(applyOfficialCatalog(document, "cmd", "account", officialCatalogSchema.parse(native))));
    expect(saved.providers[0].credentials[0].supported_models).toContain("declared-model");
    expect(saved.providers[0].credentials[0].model_catalog.supportedEndpoints).toEqual(native.supportedEndpoints);
  });

  it("keeps routes in the normal catalog application and JSON save path", () => {
    const document = {
      providers: [{ id: "cmd", preset: "commandcode-api", base_url: "https://api.commandcode.ai/provider/v1",
        credentials: [{ id: "account", supported_models: [] }], supported_models: [] }],
      model_routes: [],
    } as unknown as ConsoleRouteDocument;
    const result = applyOfficialCatalog(document, "cmd", "account", officialCatalogSchema.parse(catalog));
    const saved = JSON.parse(JSON.stringify(result));
    expect(saved.providers[0].credentials[0].model_catalog.supportedEndpoints).toEqual(catalog.supportedEndpoints);
    expect(document.providers[0]).not.toHaveProperty("credentials.0.model_catalog");
  });

  it("reads old catalogs without inventing routes", () => {
    const { supportedEndpoints: _, ...old } = catalog;
    expect(officialCatalogSchema.parse(old)).not.toHaveProperty("supportedEndpoints");
  });

  it("rejects foreign, missing, unknown and duplicate declarations", () => {
    for (const value of [
      { ...catalog, sourceUrl: catalogSources.openai },
      { ...catalog, supportedEndpoints: { other: ["/responses"] } },
      { ...catalog, supportedEndpoints: { "declared-model": ["/future"] } },
      { ...catalog, supportedEndpoints: { "declared-model": ["/responses", "/responses"] } },
    ]) expect(officialCatalogSchema.safeParse(value).success).toBe(false);
  });
});
