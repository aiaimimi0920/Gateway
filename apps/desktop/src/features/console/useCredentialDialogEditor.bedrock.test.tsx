import { writeFileSync } from "node:fs";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { buildCredentialSecretPatches, type CredentialSecretEdit } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";
import { bedrockEndpoint, type BedrockMaterial } from "./bedrockMaterial";

it("runs the ordinary editor and secret patch chain for all modes, including a later metadata-only edit", () => {
  const fixtures = ["bedrock_bearer", "aws_sigv4", "aws_sigv4_temporary"].map((mode) => {
    const active = routeDocument([{ id: "renamed", preset: "bedrock-converse", base_url: bedrockEndpoint("us-east-1", false), explicit_credentials_only: true, credentials: [] }]);
    const setError = vi.fn();
    const { result, unmount } = renderHook(() => {
      const [document, setDocument] = useState(active); const [editMode, setMode] = useState<"add" | "edit">("add");
      const [edits, setEdits] = useState<CredentialSecretEdit[]>([]);
      const editor = useCredentialDialogEditor({ editorText: JSON.stringify(document), credentialDialogState: { mode: editMode, initialValue: null },
        setCredentialDialogState: vi.fn(), setCredentialSecretEdits: setEdits, setError, replaceEditorDocument: setDocument, t: (zh) => zh });
      return { editor, document, edits, setMode };
    });
    const common = { providerId: "renamed", credentialId: "a", accountName: "", enabled: true, baseUrl: "", supportedModelsText: "fixture-model",
      fetchedModelsText: "", manualModelsText: "fixture-model", apiKeyValue: "" };
    const material: BedrockMaterial = { authMode: mode as BedrockMaterial["authMode"], region: "us-west-2", fips: false,
      apiKey: mode === "bedrock_bearer" ? "synthetic-new-bearer" : "", accessKeyId: mode !== "bedrock_bearer" ? "synthetic-new-access" : "",
      secretAccessKey: mode !== "bedrock_bearer" ? "synthetic-new-secret" : "", sessionToken: mode === "aws_sigv4_temporary" ? "synthetic-new-sts" : "" };
    act(() => result.current.editor.applyCredentialDialogValue({ ...common, apiKeyOperation: "replace", bedrockMaterial: material }));
    expect(setError).toHaveBeenLastCalledWith(null);
    act(() => result.current.setMode("edit"));
    act(() => result.current.editor.applyCredentialDialogValue({ ...common, baseUrl: bedrockEndpoint("us-west-2", false), accountName: "Metadata edit", apiKeyOperation: "keep" }));
    expect(result.current.edits.some((e) => e.operation === "replace")).toBe(true);
    expect(JSON.stringify(result.current.document)).not.toContain("synthetic-");
    expect(JSON.stringify(result.current.document)).not.toContain("discovery_job");
    expect(result.current.document.model_routes).toContainEqual(expect.objectContaining({ pattern: "fixture-model", provider_ids: ["renamed"], enabled: true }));
    const secretPatches = buildCredentialSecretPatches({ activeDocument: active, draftDocument: result.current.document, activeSecrets: [], credentialSecretEdits: result.current.edits });
    const fixture = { mode, active, document: result.current.document, secretPatches };
    unmount();
    return fixture;
  });
  const path = process.env.GATEWAY_BEDROCK_MATERIAL_FIXTURE_PATH;
  if (path) writeFileSync(path, JSON.stringify(fixtures) + "\n", "utf8");
});
it("rejects the old single-key route for Bedrock without partial document or staged edits", () => {
  const active = routeDocument([{ id: "renamed", preset: "bedrock-converse", base_url: bedrockEndpoint("us-east-1", false), explicit_credentials_only: true, credentials: [] }]);
  const replace = vi.fn(); const edits = vi.fn(); const error = vi.fn();
  const { result } = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(active), credentialDialogState: { mode: "add", initialValue: null },
    setCredentialDialogState: vi.fn(), setCredentialSecretEdits: edits, setError: error, replaceEditorDocument: replace, t: (zh) => zh }));
  act(() => result.current.applyCredentialDialogValue({ providerId: "renamed", credentialId: "a", accountName: "", enabled: true,
    baseUrl: "", supportedModelsText: "", apiKeyValue: "synthetic-wrong", apiKeyOperation: "replace" }));
  expect(error).toHaveBeenCalledWith(expect.any(String)); expect(replace).not.toHaveBeenCalled(); expect(edits).not.toHaveBeenCalled();
});
