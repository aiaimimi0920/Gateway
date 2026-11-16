import { expect, it } from "vitest";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";
import { parseProviderSessionMaterial } from "./providerSessionMaterial";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { sessionMaterialTarget, sessionAuthForProduct } from "./providerSessionTarget";

function staged(userAgent?: string) {
  const active = routeDocument([{ id: "q", preset: "qwen-web-chat", base_url: "https://chat.qwen.ai/api/v1",
    credentials: [{ id: "a" }] }]);
  const material = parseProviderSessionMaterial(JSON.stringify({ product: "qwen-web", token: "synthetic-token",
    cookieHeader: "token=synthetic-token; cf_clearance=synthetic-clearance", userAgent }), "qwen-web");
  return { active, ...applyProviderSessionMaterial(active, "q", "a", material) };
}
function patches(source: ReturnType<typeof staged>) {
  return buildCredentialSecretPatches({ activeDocument: source.active, draftDocument: source.document,
    activeSecrets: [], credentialSecretEdits: source.secretEdits });
}
it.each(["exact-UA", undefined])("keeps Qwen optional original UA with its imported token and Cookie: %s", (agent) => {
  const source = staged(agent);
  expect(patches(source)).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-token" });
  expect(JSON.stringify(source.document)).not.toContain("synthetic-token");
});
it.each(["replace", "delete", "case", "provider", "add"]) ("rejects Qwen pending session UA rebinding: %s", (change) => {
  const source = staged(change === "add" ? undefined : "exact-UA");
  const provider = source.document.providers[0] as Record<string, any>;
  const account = provider.credentials[0];
  if (change === "replace") account.headers["User-Agent"] = "foreign-UA";
  if (change === "delete") delete account.headers["User-Agent"];
  if (change === "case") account.headers["user-agent"] = "foreign-UA";
  if (change === "provider") provider.headers = { "User-Agent": "foreign-UA" };
  if (change === "add") account.headers["User-Agent"] = "foreign-UA";
  expect(() => patches(source)).toThrow("User-Agent");
});
it("keeps Perplexity UA mandatory when a binding bypasses the normal parser", () => {
  const document = routeDocument([{ id: "p", preset: "perplexity-web", base_url: "https://www.perplexity.ai",
    credentials: [{ id: "a", session_auth: sessionAuthForProduct("perplexity-web") }] }]);
  expect(() => sessionMaterialTarget(document, "p", "a", { product: "perplexity-web" }, true)).toThrow("User-Agent");
});
