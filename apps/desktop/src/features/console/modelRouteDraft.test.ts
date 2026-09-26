import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { rewriteModelReferences } from "./modelRouteDraft";

describe("model reference renaming", () => {
  it("preserves a prototype-named model as an own mapping without changing object prototypes", () => {
    const provider = {
      id: "configured-provider",
      supported_models: ["source"],
      credentials: [{ supported_models: ["source", "other"] }],
      model_map: { source: "upstream-model", other: "other-upstream" },
    };
    const document: ConsoleRouteDocument = {
      providers: [provider], model_routes: [], aliases: { answer: "source" },
    };
    const prototype = Object.getPrototypeOf(provider.model_map);

    rewriteModelReferences(document, "source", "__proto__");

    expect(Object.getOwnPropertyDescriptor(provider.model_map, "__proto__")?.value).toBe("upstream-model");
    expect(Object.getPrototypeOf(provider.model_map)).toBe(prototype);
    expect(Object.keys(provider.model_map)).toEqual(["other", "__proto__"]);
    expect(provider.supported_models).toEqual(["__proto__"]);
    expect(provider.credentials[0]?.supported_models).toEqual(["__proto__", "other"]);
    expect(document.aliases.answer).toBe("__proto__");
    expect(JSON.parse(JSON.stringify(provider.model_map))).toEqual({
      other: "other-upstream", ["__proto__"]: "upstream-model",
    });
  });
});
