import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { parseProviderSessionMaterial } from "./providerSessionMaterial";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

it.each([false, true])("uses normal editor + SecretPatch submit, rejects redirected=%s atomically", (redirected) => {
  const active = routeDocument([{ id: "renamed", preset: "freebuff", base_url: "https://freebuff.com", explicit_credentials_only: true, credentials: [] }]);
  const material = parseProviderSessionMaterial(JSON.stringify({ product: "freebuff-web", token: "synthetic-freebuff-token", agentId: "agent-fixture" }), "freebuff-web");
  const setError = vi.fn();
  const { result } = renderHook(() => {
    const [document, setDocument] = useState(active);
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode: "add", initialValue: null },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits };
  });
  act(() => result.current.editor.applyCredentialDialogValue({ providerId: "renamed", credentialId: "account", accountName: "Session fixture", enabled: true,
    baseUrl: redirected ? "https://evil.test" : "", supportedModelsText: "fixture-model", apiKeyOperation: "replace", apiKeyValue: "", sessionMaterial: material }));
  if (redirected) {
    expect(setError).toHaveBeenCalledWith(expect.any(String)); expect(result.current.document).toEqual(active); expect(result.current.edits).toEqual([]); return;
  }
  expect(JSON.stringify(result.current.document)).not.toContain("synthetic-freebuff-token");
  expect(result.current.document.providers[0]).toMatchObject({ explicit_credentials_only: true, credentials: [{ extra_body: { freebuffAgentId: "agent-fixture" }, supported_models: ["fixture-model"] }] });
  expect(JSON.stringify(result.current.document)).not.toContain("discovery_job");
  const patches = buildCredentialSecretPatches({ activeDocument: active, draftDocument: result.current.document, activeSecrets: [], credentialSecretEdits: result.current.edits });
  expect(patches).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-freebuff-token" });
});

it("preserves staged session secrets through a later metadata-only edit", () => {
  const active = routeDocument([{ id: "web", preset: "grok", base_url: "https://grok.com", explicit_credentials_only: true, credentials: [] }]);
  const material = parseProviderSessionMaterial(JSON.stringify({ product: "grok-web", token: "synthetic-grok" }), "grok-web");
  const { result } = renderHook(() => {
    const [document, setDocument] = useState(active); const [mode, setMode] = useState<"add" | "edit">("add");
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode, initialValue: null },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: vi.fn(), replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits, setMode };
  });
  const common = { providerId: "web", credentialId: "a", accountName: "", enabled: true, baseUrl: "", supportedModelsText: "fixture-model", apiKeyValue: "" };
  act(() => result.current.editor.applyCredentialDialogValue({ ...common, apiKeyOperation: "replace", sessionMaterial: material }));
  act(() => result.current.setMode("edit"));
  act(() => result.current.editor.applyCredentialDialogValue({ ...common, accountName: "renamed", apiKeyOperation: "keep" }));
  expect(result.current.edits).toContainEqual(expect.objectContaining({ field: "api_key", value: "synthetic-grok" }));
  expect(JSON.stringify(result.current.document)).not.toContain("synthetic-grok");
  expect(buildCredentialSecretPatches({ activeDocument: active, draftDocument: result.current.document, activeSecrets: [], credentialSecretEdits: result.current.edits }))
    .toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-grok" });
});
