import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { readFileSync, writeFileSync } from "node:fs";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { parseChatgptWebMaterial } from "./chatgptWebMaterial";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

it("stages Web metadata and all secret patches through the normal editor owner", () => {
  const active = routeDocument([{ id: "web", preset: "chatgpt-web-reverse", base_url: "https://chatgpt.com", credentials: [],
    supported_models: [], auto_refill_enabled: false, auto_prune_enabled: false }]);
  const material = parseChatgptWebMaterial(JSON.stringify({ accessToken: "web-persistence-fixture", cookieHeader: "session=web-cookie-fixture",
    expiresAt: "2099-01-01T00:00:00Z", userAgent: "Browser/1", deviceId: "device", sessionId: "web-session-private-fixture" }));
  const { result } = renderHook(() => {
    const [document, setDocument] = useState(active);
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode: "add", initialValue: null },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: vi.fn(),
      replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits };
  });
  act(() => result.current.editor.applyCredentialDialogValue({ providerId: "web", credentialId: "web-account", accountName: "Web fixture",
    enabled: false, baseUrl: "", supportedModelsText: "gpt-5.4", apiKeyOperation: "replace", apiKeyValue: material.accessToken, webSessionMaterial: material }));
  expect(result.current.edits).toHaveLength(3);
  expect(JSON.stringify(result.current.document)).not.toContain("web-persistence-fixture");
  expect(JSON.stringify(result.current.document)).not.toContain("web-cookie-fixture");
  expect(JSON.stringify(result.current.document)).not.toContain("web-session-private-fixture");
  expect(JSON.stringify(result.current.document)).not.toContain("discovery_job");
  const secretPatches = buildCredentialSecretPatches({ activeDocument: active, draftDocument: result.current.document, activeSecrets: [], credentialSecretEdits: result.current.edits });
  expect(secretPatches).toHaveLength(3);
  // Explicit opt-in export contains only synthetic fixtures for the isolated restart verifier.
  const path = process.env.GATEWAY_WEB_MATERIAL_FIXTURE_PATH;
  if (path) writeFileSync(path, JSON.stringify({ document: result.current.document, secretPatches }), "utf8");
});

it.skipIf(!process.env.GATEWAY_WEB_MATERIAL_INPUT_PATH)("accepts an explicitly supplied private worker sample without printing it", () => {
  const material = parseChatgptWebMaterial(readFileSync(process.env.GATEWAY_WEB_MATERIAL_INPUT_PATH!, "utf8"));
  expect(Boolean(material.accessToken && material.cookieHeader && material.userAgent)).toBe(true);
  expect(Date.parse(material.expiresAt) > Date.now()).toBe(true);
});
