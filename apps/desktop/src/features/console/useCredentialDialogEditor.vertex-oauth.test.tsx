import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google";
const value = (): CredentialDialogValue => ({ providerId: "renamed", credentialId: "stable", accountName: "Operator label", enabled: false, baseUrl: "",
  supportedModelsText: "gemini-contract", manualModelsText: "gemini-contract", fetchedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "" });
function setup(mode: "add" | "edit" = "edit") {
  const context = { grant_type: "desktop_authorization_code", client_id: "explicit-client", client_secret: "",
    requested_scope: "https://www.googleapis.com/auth/cloud-platform", scope_reported: true,
    target: { project: "project-a", location: "global", quota_project: "quota-a" } };
  const active = routeDocument([{ id: "renamed", preset: "google-agent-platform-official-api", auth_mode: "vertex_user_oauth", base_url: base,
    explicit_credentials_only: true, credentials: [{ id: "stable", vertex_oauth: context, api_key: "", refresh_token: "", refresh_client_id: "explicit-client",
      refresh_endpoint: "https://oauth2.googleapis.com/token", expires_at: "2030-01-01T00:00:00Z", enabled: true }] }]);
  const error = vi.fn(); const activeSecrets = ["api_key", "refresh_token", "vertex_oauth/client_secret"].map((field) => ({ path: `/providers/0/credentials/0/${field}`, configured: true }));
  const hook = renderHook(() => {
    const [document, setDocument] = useState(active); const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode, initialValue: value() },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits };
  });
  return { ...hook, active, error, activeSecrets };
}
it("keeps nested secret descriptors and stable OAuth identity when saving metadata/manual models", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(value()));
  expect(h.error).toHaveBeenLastCalledWith(null); expect(h.result.current.edits).toEqual([]);
  const provider = h.result.current.document.providers[0] as Record<string, unknown>;
  const credential = (provider.credentials as Record<string, unknown>[])[0];
  const original = ((h.active.providers[0] as Record<string, unknown>).credentials as Record<string, unknown>[])[0];
  expect(credential).toMatchObject({ id: "stable", enabled: false, account_name: "Operator label", vertex_oauth: original.vertex_oauth,
    refresh_client_id: original.refresh_client_id, refresh_endpoint: original.refresh_endpoint, expires_at: original.expires_at });
  const patches = buildCredentialSecretPatches({ activeDocument: h.active, draftDocument: h.result.current.document,
    activeSecrets: h.activeSecrets, credentialSecretEdits: h.result.current.edits });
  expect(patches).toEqual(expect.arrayContaining(h.activeSecrets.map(({ path }) => ({ path, operation: "keep" }))));
  expect(patches.every((entry) => entry.operation === "keep")).toBe(true);
});
it.each([{ apiKeyOperation: "replace" as const, apiKeyValue: "manual" }, { apiKeyOperation: "clear" as const },
  { baseUrl: "https://untrusted.invalid" }, { manualModelsText: "models/full-path" }, { fetchedModelsText: "borrowed" }])("rejects generic material/target/model mutations without editing the document: %j", (patch) => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue({ ...value(), ...patch }));
  expect(h.error).not.toHaveBeenLastCalledWith(null); expect(h.result.current.document).toEqual(h.active); expect(h.result.current.edits).toEqual([]);
});
it("cannot add a Full Vertex account through the generic credential editor", () => {
  const h = setup("add"); act(() => h.result.current.editor.applyCredentialDialogValue(value()));
  expect(h.result.current.document).toEqual(h.active); expect(h.error).not.toHaveBeenLastCalledWith(null);
});
