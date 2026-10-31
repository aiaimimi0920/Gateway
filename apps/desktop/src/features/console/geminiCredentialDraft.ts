import type {
  ConsoleGeminiAuthSession,
  ConsoleGeminiGeneratedCredentialDraft,
  ConsoleRouteDocument,
} from "../../api/contracts";
import {
  addExplicitCredential,
  updateExplicitCredential,
  type CredentialSecretEdit,
} from "./credentialDocument";
import { isRecord } from "./routeDocument";
import { applyModelSelection, modelSelectionSchema } from "./credentialModelSelection";
import { separateGeminiWebDraftSecrets } from "./geminiWebDraftSecrets";

// Manual routing defaults for the working host surfaces, not a fetched model directory.
const canvasDefaultModels = ["gemini-3-flash-preview", "gemini-2.5-flash-image", "gemini-2.5-flash-preview-tts"];

export function isGeminiAuthSessionTerminal(status: ConsoleGeminiAuthSession["status"]): boolean {
  return status === "succeeded" || status === "failed";
}

export function canManuallyCompleteGeminiAuthSession(
  session: ConsoleGeminiAuthSession | null,
): boolean {
  if (!session || session.status !== "waiting_user") {
    return false;
  }
  return (
    session.targetFamily === "gemini-canvas" || session.targetFamily === "gemini-canvas-chat"
  );
}


export function applyGeminiGeneratedDraftsToDocument(
  document: ConsoleRouteDocument,
  generatedDrafts: ConsoleGeminiGeneratedCredentialDraft[],
): {
  document: ConsoleRouteDocument;
  secretEdits: CredentialSecretEdit[];
} {
  let nextDocument = document;
  const secretEdits: CredentialSecretEdit[] = [];

  for (const draft of generatedDrafts) {
    const credentialIdValue = draft.credential.id;
    if (typeof credentialIdValue !== "string" || credentialIdValue.trim().length === 0) {
      throw new Error("Generated Gemini credential is missing a stable id.");
    }
    const credentialId = credentialIdValue.trim();
    const provider = nextDocument.providers.find(
      (entry) => isRecord(entry) && entry.id === draft.providerId,
    );
    if (!isRecord(provider)) {
      throw new Error(`Provider '${draft.providerId}' could not be found.`);
    }

    const web = ["gemini-web-chat", "gemini-web-chat-modular"].includes(String(provider.preset));
    const canvas = provider.preset === "gemini-canvas-browser-relay";
    const separated = web ? separateGeminiWebDraftSecrets(draft.credential, draft.providerId, credentialId) : null;
    const updates = { ...(separated?.credential ?? draft.credential), id: credentialId };
    secretEdits.push(...(separated?.secretEdits ?? []));
    const existingCredential =
      Array.isArray(provider.credentials) &&
      provider.credentials.some((entry) => isRecord(entry) && entry.id === credentialId);
    nextDocument = existingCredential
      ? updateExplicitCredential(
          nextDocument,
          {
            providerId: draft.providerId,
            credentialId,
          },
          updates,
        )
      : addExplicitCredential(nextDocument, {
          providerId: draft.providerId,
          credential: updates,
        });

    if (web || canvas) {
      const nextProvider = nextDocument.providers.find((entry) => isRecord(entry) && entry.id === draft.providerId);
      const credential = isRecord(nextProvider) && Array.isArray(nextProvider.credentials)
        ? nextProvider.credentials.find((entry) => isRecord(entry) && entry.id === credentialId) : null;
      if (isRecord(credential)) {
        const models = Array.isArray(credential.supported_models)
          ? credential.supported_models.filter((entry): entry is string => typeof entry === "string") : [];
        // Manual defaults are routing hints, never claims of a discovered/verified model directory.
        const selection = credential.model_selection === undefined
          ? { fetched: [], manual: models.length ? models : canvas ? canvasDefaultModels : ["gemini-web-chat"] }
          : modelSelectionSchema.parse(credential.model_selection);
        nextDocument = applyModelSelection(nextDocument, draft.providerId, credentialId, selection);
      }
    }

    for (const secretEdit of draft.secretEdits) {
      if (
        ["api_key", "auth_token", "headers/Cookie"].includes(secretEdit.field) &&
        secretEdit.operation === "replace"
      ) {
        secretEdits.push({
          providerId: draft.providerId,
          credentialId,
          field: secretEdit.field,
          operation: "replace",
          value: secretEdit.value,
        });
      }
    }
  }

  return { document: nextDocument, secretEdits };
}
