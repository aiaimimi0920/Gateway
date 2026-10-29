import { describe, expect, it } from "vitest";
import { parseChatgptWebMaterial, applyChatgptWebMaterial } from "./chatgptWebMaterial";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const sample = () => ({ authToken: "web-access-fixture", expiresAt: "2099-01-01T00:00:00+00:00",
  cookieHeader: "session=web-cookie-fixture", userAgent: "Browser/1", deviceId: "device", sessionId: "session",
  clientVersion: "v1", language: "zh-CN", debug: { token: "do-not-save" } });

describe("independent ChatGPT Web session import", () => {
  it("normalizes worker material and drops debug/raw fields", () => {
    const material = parseChatgptWebMaterial(JSON.stringify(sample()));
    expect(material.expiresAt).toBe("2099-01-01T00:00:00.000Z");
    expect(material.extraBody).toMatchObject({ deviceId: "device", clientVersion: "v1" });
    expect(material.sessionId).toBe("session");
    expect(JSON.stringify(material)).not.toContain("do-not-save");
  });

  it("uses token expiry rather than the longer browser session expiry", () => {
    const exp = 4070908800;
    const accessToken = `e30.${btoa(JSON.stringify({ exp })).replaceAll("=", "")}.sig`;
    const material = parseChatgptWebMaterial(JSON.stringify({ ...sample(), authToken: accessToken, expiresAt: "2099-01-01T00:00:00Z" }));
    expect(material.expiresAt).toBe(new Date(exp * 1000).toISOString());
  });

  it.each([
    { access_token: "codex-token", refresh_token: "codex-refresh" },
    { ...sample(), cookieHeader: "x=y\r\nInjected: yes" },
    { ...sample(), expiresAt: "not-a-date" },
    { ...sample(), expiresAt: undefined, expires: "2099-01-01T00:00:00Z" },
    { ...sample(), userAgent: "" },
    { ...sample(), authToken: "a".repeat(131073) },
  ])("rejects incomplete/unsafe material without echoing input", (input) => {
    expect(() => parseChatgptWebMaterial(JSON.stringify(input))).toThrow();
    try { parseChatgptWebMaterial(JSON.stringify(input)); } catch (error) {
      expect(String(error)).not.toContain("codex-token");
      expect(String(error)).not.toContain("Injected");
    }
  });

  it("keeps secrets out of the document and rebases secret patches by identity", () => {
    const document = routeDocument([{ id: "web", preset: "chatgpt-web-reverse", credentials: [{ id: "account", headers: { Accept: "text/event-stream" } }] }]);
    const result = applyChatgptWebMaterial(document, "web", "account", parseChatgptWebMaterial(JSON.stringify(sample())));
    const serialized = JSON.stringify(result.document);
    expect(serialized).not.toContain("web-access-fixture");
    expect(serialized).not.toContain("web-cookie-fixture");
    expect(serialized).toContain("text/event-stream");
    result.document.providers.unshift({ id: "other", credentials: [] });
    const patches = buildCredentialSecretPatches({ activeDocument: document, draftDocument: result.document,
      activeSecrets: [], credentialSecretEdits: result.secretEdits });
    expect(patches).toContainEqual({ path: "/providers/1/credentials/0/headers/Cookie", operation: "replace", value: "session=web-cookie-fixture" });
    expect(patches).toContainEqual({ path: "/providers/1/credentials/0/api_key", operation: "replace", value: "web-access-fixture" });
    expect(JSON.stringify(document)).not.toContain("Browser/1");
  });

  it("refuses to import Web materials into Codex OAuth", () => {
    const document = routeDocument([{ id: "codex", preset: "chatgpt-codex-oauth-official-api", credentials: [{ id: "account" }] }]);
    expect(() => applyChatgptWebMaterial(document, "codex", "account", parseChatgptWebMaterial(JSON.stringify(sample())))).toThrow();
  });

  it("replaces an existing lower-case cookie without keeping a stale duplicate", () => {
    const document = routeDocument([{ id: "web", preset: "chatgpt-web-reverse", credentials: [{ id: "account", headers: {} }] }]);
    const result = applyChatgptWebMaterial(document, "web", "account", parseChatgptWebMaterial(JSON.stringify({ ...sample(), timezone: null })));
    const patches = buildCredentialSecretPatches({ activeDocument: document, draftDocument: result.document,
      activeSecrets: [{ path: "/providers/0/credentials/0/headers/cookie", configured: true }], credentialSecretEdits: result.secretEdits });
    expect(patches.filter((entry) => entry.path.toLowerCase().endsWith("/headers/cookie"))).toHaveLength(1);
  });

  it("accepts a worker without sessionId and clears stale session material", () => {
    const document = routeDocument([{ id: "web", preset: "chatgpt-web-reverse", credentials: [{ id: "account" }] }]);
    const material = parseChatgptWebMaterial(JSON.stringify({ ...sample(), sessionId: null }));
    const result = applyChatgptWebMaterial(document, "web", "account", material);
    expect(result.secretEdits).toContainEqual({ providerId: "web", credentialId: "account", field: "extra_body/sessionId", operation: "clear" });
  });
});
