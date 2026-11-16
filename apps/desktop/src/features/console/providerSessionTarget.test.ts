import { expect, it } from "vitest";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { SESSION_PRODUCTS, type SessionProduct } from "./providerSessionProducts";
import { sessionMaterialTarget } from "./providerSessionTarget";

it.each(Object.keys(SESSION_PRODUCTS) as SessionProduct[])("accepts serialized optional null addresses for %s without weakening the audience", product => {
  const definition = SESSION_PRODUCTS[product];
  for (const credentialOverride of [false, true]) {
    const provider = { id: "p", preset: definition.preset, base_url: credentialOverride ? null : definition.baseUrl,
      credentials: [{ id: "a", base_url: credentialOverride ? definition.baseUrl : null }] };
    expect(sessionMaterialTarget(routeDocument([provider]), "p", "a", { product }).credential.id).toBe("a");
  }
});

it.each(["", 1, false, "https://foreign.invalid", "https://studio-api-prod.suno.com?redirect=foreign"])("rejects an explicit non-null invalid address %s", invalid => {
  const provider = { id: "p", preset: "suno", base_url: "https://studio-api-prod.suno.com",
    credentials: [{ id: "a", base_url: invalid }] };
  expect(() => sessionMaterialTarget(routeDocument([provider]), "p", "a", { product: "suno-web" })).toThrow("固定入口");
});

it("rejects a missing audience when neither owner has an address", () => {
  const document = routeDocument([{ id: "p", preset: "suno", base_url: null, credentials: [{ id: "a", base_url: null }] }]);
  expect(() => sessionMaterialTarget(document, "p", "a", { product: "suno-web" })).toThrow("固定入口");
});
