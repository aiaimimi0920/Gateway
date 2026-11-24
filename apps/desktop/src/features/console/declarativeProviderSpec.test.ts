import { describe, expect, it } from "vitest";
import { defineDeclarativeProvider } from "./declarativeProviderSpec";

describe("defineDeclarativeProvider", () => {
  const sample = defineDeclarativeProvider({
    id: "joycode-api",
    preset: "joycode-official-api",
    vendorKey: "joycode",
    vendorName: "JoyCode",
    descriptionZh: "JoyCode 官方编程接口。",
    descriptionEn: "JoyCode official coding API.",
    compatibility: "native",
    defaultBaseUrl: "https://api.joycode.example/v1/",
    modelsPath: "/models",
    extraFields: [
      {
        key: "workspaceId",
        labelZh: "工作空间",
        labelEn: "Workspace",
        kind: "text",
        required: false,
      },
    ],
  });

  it("normalizes default base URL, models endpoint, and template defaults", () => {
    expect(sample.template.baseUrl).toBe("https://api.joycode.example/v1");
    expect(sample.template.providerId).toBe("joycode-api");
    expect(sample.modelsCatalogUrl).toBe("https://api.joycode.example/v1/models");
    expect(sample.credentialFields.map((f) => f.key)).toEqual(["apiKey", "workspaceId"]);
  });

  it("enforces strict HTTPS origin matching and blocks lookalike, query, or plaintext URLs", () => {
    expect(sample.isApprovedBaseUrl("https://api.joycode.example/v1")).toBe(true);
    expect(sample.isApprovedBaseUrl("https://api.joycode.example/v1/")).toBe(true);
    for (const bad of [
      "",
      "http://api.joycode.example/v1",
      "https://api.joycode.example/v1?key=secret",
      "https://api.joycode.example/v1#frag",
      "https://user:pw@api.joycode.example/v1",
      "https://api.joycode.example.evil.invalid/v1",
    ]) {
      expect(sample.isApprovedBaseUrl(bad)).toBe(false);
    }
  });
});
