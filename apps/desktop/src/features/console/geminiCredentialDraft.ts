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

    const updates = { ...draft.credential, id: credentialId };
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

    for (const secretEdit of draft.secretEdits) {
      if (
        (secretEdit.field === "api_key" || secretEdit.field === "auth_token") &&
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
