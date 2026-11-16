import { expect, it } from "vitest";
import { parseProviderSessionMaterial } from "./providerSessionMaterial";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

function staged() {
  const active = routeDocument([{ id: "f", preset: "freebuff", base_url: "https://freebuff.com", credentials: [{ id: "a" }] }]);
  const material = parseProviderSessionMaterial(JSON.stringify({ product: "freebuff-web", token: "synthetic-freebuff", agentId: "agent-fixture" }), "freebuff-web");
  return { active, ...applyProviderSessionMaterial(active, "f", "a", material) };
}
it.each(["provider-map", "credential-map", "refresh-endpoint", "refresh-client", "session-auth", "expiry", "callFlow", "authMode"])("refuses pending material retargeting through %s", (change) => {
  const result = staged(); const provider = result.document.providers[0] as Record<string, any>; const credential = provider.credentials[0];
  if (change === "provider-map") provider.extra_body = { freebuffModelAgentMap: { model: "other-agent" } };
  if (change === "credential-map") credential.extra_body.freebuffModelAgentMap = { model: "other-agent" };
  if (change === "refresh-endpoint") credential.refresh_endpoint = "https://evil.test/token";
  if (change === "refresh-client") credential.refresh_client_id = "other-client";
  if (change === "session-auth") credential.session_auth = { transport: "cookie", primary_cookie_name: "other-session" };
  if (change === "expiry") credential.expires_at = "2099-01-01T00:00:00Z";
  if (change === "callFlow") provider.callFlow = { special_adapter: "grok_web" };
  if (change === "authMode") provider.authMode = "none";
  expect(() => buildCredentialSecretPatches({ activeDocument: result.active, draftDocument: result.document, activeSecrets: [], credentialSecretEdits: result.secretEdits })).toThrow();
});
it("clears an old refresh token instead of retaining an unrelated OAuth identity", () => {
  expect(staged().secretEdits).toContainEqual(expect.objectContaining({ field: "refresh_token", operation: "clear" }));
});
