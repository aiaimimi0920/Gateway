import { writeFileSync } from "node:fs";
import { expect, it } from "vitest";
import { parseProviderSessionMaterial } from "./providerSessionMaterial";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";
import { SESSION_PRODUCTS, type SessionProduct } from "./providerSessionProducts";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { buildCredentialSecretPatches } from "./credentialDocument";

it("projects all eight synthetic products into real backend roundtrip fixtures", () => {
  const fixtures = Object.entries(SESSION_PRODUCTS).map(([id, product]) => {
    const productId = id as SessionProduct;
    const active = routeDocument([{ id: "renamed", preset: product.preset, base_url: product.baseUrl, explicit_credentials_only: true,
      credentials: [id === "perplexity-web" ? { id: "account", api_key: "synthetic-old-session", headers: { "User-Agent": "old-UA", Cookie: "__Secure-next-auth.session-token=synthetic-old-session" },
        session_auth: { transport: "cookie", primary_cookie_name: "__Secure-next-auth.session-token" } }
        : { id: "account", refresh_token: "synthetic-old-refresh", headers: { cookie: "old=synthetic-cookie", authorization: "Bearer synthetic-old" } }] }]);
    const draft = structuredClone(active);
    (draft.providers[0] as Record<string, any>).credentials[0].headers = {};
    delete (draft.providers[0] as Record<string, any>).credentials[0].refresh_token;
    const material = parseProviderSessionMaterial(JSON.stringify({ product: id, token: "synthetic-new-token",
      ...(id === "perplexity-web" ? { userAgent: "synthetic-agent" } : {}),
      ...(id === "suno-web" ? { cookieHeader: "__session=synthetic-new-token; other=synthetic" } : {}),
      ...(id === "lumalabs-web" ? { realmId: "synthetic-board" } : {}), ...(id === "freebuff-web" ? { agentId: "synthetic-agent" } : {}) }), productId);
    const imported = applyProviderSessionMaterial(draft, "renamed", "account", material);
    imported.document.providers.unshift({ id: "other", base_url: "https://fixture.test", explicit_credentials_only: true, credentials: [] });
    const secretPatches = buildCredentialSecretPatches({ activeDocument: active, draftDocument: imported.document,
      activeSecrets: id === "perplexity-web" ? ["api_key", "headers/Cookie"].map((key) => ({ path: `/providers/0/credentials/0/${key}`, configured: true }))
        : [...["cookie", "authorization"].map((key) => ({ path: `/providers/0/credentials/0/headers/${key}`, configured: true })),
          { path: "/providers/0/credentials/0/refresh_token", configured: true }], credentialSecretEdits: imported.secretEdits });
    expect(JSON.stringify(imported.document)).not.toContain("synthetic-new-token");
    if (id !== "perplexity-web") expect(secretPatches).toContainEqual({ path: "/providers/1/credentials/0/headers/cookie", operation: "clear" });
    expect(secretPatches).toContainEqual({ path: "/providers/1/credentials/0/refresh_token", operation: "clear" });
    return { product: id, active, document: imported.document, secretPatches };
  });
  expect(fixtures).toHaveLength(8);
  // Opt-in only, synthetic values only; never read historical/private credentials from this test.
  const path = process.env.GATEWAY_SESSION_MATERIAL_FIXTURE_PATH;
  if (path) writeFileSync(path, `${JSON.stringify(fixtures)}\n`, "utf8");
});
