import { expect, it } from "vitest";
import { displayLabel } from "./displayLabel";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { parseRouteDocument } from "./routeDocument";
import { providerBrandLabel } from "./providerBrandLabel";

const misdecode = (value: string) => Array.from(new TextEncoder().encode(value),
  (byte) => String.fromCharCode(byte)).join("");

it("recovers single and repeated legacy Latin-1 decoding for display", () => {
  const label = "Muyuan · 第三方 OpenAI 兼容";
  expect(displayLabel(misdecode(label))).toBe(label);
  expect(displayLabel(misdecode(misdecode(label)))).toBe(label);
  expect(displayLabel(misdecode(misdecode(misdecode(label))))).toBe(label);
});

it("preserves normal Unicode, invalid bytes and oversized labels without guessing", () => {
  for (const label of ["我的自定义站点", "Ångström", "Âge", "François", "OpenAI", "站点 🚀",
    String.fromCharCode(195, 40), "unknown �", "x".repeat(4097)]) {
    expect(displayLabel(label)).toBe(label);
  }
});

it("repairs custom provider display and test scopes without changing the document", () => {
  const label = "自定义服务商 · 兼容 API";
  const corrupted = misdecode(misdecode(label));
  expect(providerBrandLabel({ providerId: "custom-relay", providerLabel: corrupted })).toBe(label);
  const document = parseRouteDocument(JSON.stringify({
    providers: [{ id: "custom-relay", label: corrupted }], model_routes: [], aliases: {},
  }));
  const before = JSON.stringify(document);
  const context = testProviderContext(document, "custom-relay");
  expect(context?.options[0].label).toBe(`凭据池 · ${label}`);
  expect(context?.accounts[0].id).toBe("custom-relay::default");
  expect(JSON.stringify(document)).toBe(before);
});
