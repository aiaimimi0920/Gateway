// Staged CLI Keys stay in the secret edit owner and remain bound across later metadata edits.
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import type { CredentialDialogValue } from "./CredentialDialog";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

const base = "https://runtime.us-east-1.kiro.dev";
const draft = (apiKeyOperation: CredentialDialogValue["apiKeyOperation"]): CredentialDialogValue => ({
  providerId: "renamed", credentialId: "a", accountName: "", enabled: true, baseUrl: "",
  supportedModelsText: "Exact.Model/Version", manualModelsText: "Exact.Model/Version", fetchedModelsText: "",
  apiKeyOperation, apiKeyValue: apiKeyOperation === "replace" ? "synthetic-staged-key" : "",
});
function setup() {
  const active = routeDocument([{ id: "renamed", preset: "kiro-api-key", base_url: base, explicit_credentials_only: true, credentials: [] }]);
  const error = vi.fn();
  const hook = renderHook(() => {
    const [document, setDocument] = useState(active);
    const [mode, setMode] = useState<"add" | "edit">("add");
    const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
    const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode, initialValue: null },
      setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError: error, replaceEditorDocument: setDocument, t: (zh) => zh });
    return { editor, document, edits, setMode };
  });
  const patches = (document: ConsoleRouteDocument = hook.result.current.document, strictSessionBindings = true) => buildCredentialSecretPatches({
    activeDocument: active, draftDocument: document, activeSecrets: [], credentialSecretEdits: hook.result.current.edits, strictSessionBindings,
  });
  return { ...hook, active, error, patches };
}
it("binds a static Key replacement without putting its value, refresh or discovery into the route document", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  expect(h.error).toHaveBeenLastCalledWith(null);
  expect(h.result.current.edits).toEqual([expect.objectContaining({ operation: "replace", kiroKeyBinding: { product: "kiro-api-key", baseUrl: base, authMode: "kiro_api_key" } })]);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-key" }]);
  expect(JSON.stringify(h.result.current.document)).not.toMatch(/synthetic-|discovery_job|refresh_token|kiro_oauth/);
  h.unmount();
});
it("preserves the first staged Key through a metadata-only Keep before autosave", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  const staged = structuredClone(h.result.current.edits);
  act(() => h.result.current.setMode("edit"));
  act(() => h.result.current.editor.applyCredentialDialogValue({ ...draft("keep"), accountName: "Metadata edit" }));
  expect(h.result.current.edits).toEqual(staged);
  expect(h.patches()).toHaveLength(1); h.unmount();
});
it("rejects changing the product or effective destination of a staged Key, including non-strict preview", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  for (const patch of [{ preset: "openai", base_url: "https://untrusted.invalid" }, { base_url: "https://runtime.eu-central-1.kiro.dev" }, { auth_mode: "bearer" }]) {
    const changed = structuredClone(h.result.current.document);
    Object.assign(changed.providers[0] as Record<string, unknown>, patch);
    expect(() => h.patches(changed)).toThrow(); expect(h.patches(changed, false)).toEqual([]);
  }
  h.unmount();
});
it("requires a binding for ordinary Key replacements but leaves generic API edits unchanged", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  const edits: CredentialSecretEdit[] = [{ providerId: "renamed", credentialId: "a", field: "api_key", operation: "replace", value: "synthetic-unbound" }];
  const create = (document: ConsoleRouteDocument) => buildCredentialSecretPatches({ activeDocument: h.active,
    draftDocument: document, activeSecrets: [], credentialSecretEdits: edits });
  expect(() => create(h.result.current.document)).toThrow();
  const generic = structuredClone(h.result.current.document); (generic.providers[0] as Record<string, unknown>).preset = "openai";
  expect(create(generic)).toHaveLength(1); h.unmount();
});
it("rebases by stable IDs and replaces pending material with a normal clear", () => {
  const h = setup(); act(() => h.result.current.editor.applyCredentialDialogValue(draft("replace")));
  const reordered = structuredClone(h.result.current.document);
  reordered.providers.unshift({ id: "other" });
  ((reordered.providers[1] as Record<string, unknown>).credentials as unknown[]).unshift({ id: "sibling" });
  expect(h.patches(reordered)).toEqual([{ path: "/providers/1/credentials/1/api_key", operation: "replace", value: "synthetic-staged-key" }]);
  act(() => h.result.current.setMode("edit")); act(() => h.result.current.editor.applyCredentialDialogValue(draft("clear")));
  expect(h.result.current.edits).toEqual([{ providerId: "renamed", credentialId: "a", field: "api_key", operation: "clear" }]);
  expect(h.patches()).toEqual([{ path: "/providers/0/credentials/0/api_key", operation: "clear" }]); h.unmount();
});
