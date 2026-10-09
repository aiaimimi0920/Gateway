import { describe, expect, it } from "vitest";
import { rankedMappingModels, readProviderModelMappings, writeProviderModelMappings } from "./providerModelMappingDocument";
import { EMPTY_CONSOLE_TELEMETRY } from "./telemetry";
import { removeProviderDocument } from "./providerRemovalDocument";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { buildCredentialSecretPatches } from "./credentialDocument";

describe("pool mapping and removal documents", () => {
  it("round trips one and multiple targets, deduplicates, and removes cleared maps", () => {
    const provider = { model_map: { obsolete: "old" } } as Record<string, unknown>;
    const entries = [{ model: "a", upstreamModel: "b" }, { model: "a", upstreamModel: "c" }, { model: "single", upstreamModel: "b" }];
    writeProviderModelMappings(provider, [...entries, entries[0]]);
    expect(provider.model_map).toEqual({ single: "b" });
    expect(provider.model_map_targets).toEqual({ a: ["b", "c"] });
    expect(readProviderModelMappings(provider)).toEqual([entries[2], ...entries.slice(0, 2)]);
    writeProviderModelMappings(provider, []);
    expect(provider).toEqual({});
  });
  it("uses real local popularity before the fallback catalogue", () => {
    const models = rankedMappingModels(["custom-model"], { ...EMPTY_CONSOLE_TELEMETRY,
      retainedModelTotals: [{ providerAccountId: "p", credentialRef: "key", model: "custom-model", requestCount: 10, successCount: 9 }] });
    expect(models[0]).toBe("custom-model");
    expect(models.indexOf("gpt-6-astra")).toBeLessThan(models.indexOf("claude-opus-4-6"));
  });
  it("removes the entire pool and references once without changing other pools or input", () => {
    const document: ConsoleRouteDocument = {
      providers: [{ id: "p", credentials: [{ id: "one" }, { id: "two" }], model_map: { a: "b" } }, { id: "other", credentials: [{ id: "keep" }] }],
      model_routes: [{ pattern: "a", provider_ids: ["p"] }, { pattern: "b", provider_ids: ["p", "other"] }],
      aliases: { a: "b" }, account_groups: [{ id: "g", provider_credential_ids: ["one", "two", "p::default", "keep"] }],
    };
    const original = structuredClone(document);
    const saved = removeProviderDocument(document, "p");
    expect(saved.providers).toEqual([document.providers[1]]);
    expect(saved.model_routes).toEqual([{ pattern: "b", provider_ids: ["other"] }]);
    expect(saved.account_groups).toEqual([{ id: "g", provider_credential_ids: ["keep"] }]);
    expect(document).toEqual(original);
    expect(buildCredentialSecretPatches({ activeDocument: document, draftDocument: saved,
      activeSecrets: [{ path: "/providers/0/credentials/0/api_key", configured: true }, { path: "/providers/1/credentials/0/api_key", configured: true }],
    })).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "keep" }]);
    expect(() => removeProviderDocument(document, "missing")).toThrow("was not found");
  });
});
