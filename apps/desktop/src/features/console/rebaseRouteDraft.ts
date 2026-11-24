import type { ConsoleRouteConfigCommitRequest, ConsoleRouteConfigResponse, ConsoleRouteDocument, ConsoleSecretDescriptor, ConsoleSecretPatch } from "../../api/contracts";
import { buildCredentialSecretPatches } from "./credentialDocument";
import { isRecord } from "./routeDocument";
import { mergeRouteValue, ROUTE_EDIT_CONFLICT, sameRouteValue } from "./routeDraftMerge";

type Config = ConsoleRouteConfigResponse["routeConfig"];
const conflict = (): never => { throw new Error(ROUTE_EDIT_CONFLICT); };
const fieldIdentity = (secret: ConsoleSecretDescriptor | undefined) => secret?.configured
  ? { configured: true, fingerprint: secret.fingerprint ?? null } : { configured: false };

function remap(source: ConsoleRouteDocument, target: ConsoleRouteDocument, patches: ConsoleSecretPatch[]): ConsoleSecretPatch[] {
  return buildCredentialSecretPatches({ activeDocument: source, draftDocument: target,
    activeSecrets: patches.map(patch => ({ path: patch.path, configured: true })), activeSecretPatches: patches });
}

function ownerContext(document: ConsoleRouteDocument, path: string): unknown {
  const match = /^\/providers\/(\d+)(?:\/credentials\/(\d+))?\//.exec(path);
  if (!match) return document;
  const provider = document.providers[Number(match[1])];
  if (!isRecord(provider)) return undefined;
  const { credentials, ...settings } = provider;
  return match[2] === undefined ? { settings, accounts: credentials }
    : { settings, account: Array.isArray(credentials) ? credentials[Number(match[2])] : undefined };
}

function crossesNestedArray(document: ConsoleRouteDocument, path: string): boolean {
  let value: unknown = document;
  let parent = "";
  for (const encoded of path.slice(1).split("/")) {
    if (Array.isArray(value) && parent !== "/providers" && !/^\/providers\/\d+\/credentials$/.test(parent)) return true;
    const key = encoded.replaceAll("~1", "/").replaceAll("~0", "~");
    value = value && typeof value === "object" && Object.hasOwn(value, key) ? (value as Record<string, unknown>)[key] : undefined;
    parent += `/${encoded}`;
  }
  return false;
}

// Only display, scheduling and model choices are independent of a rotated secret's destination.
function secretBinding(context: unknown): unknown {
  if (!isRecord(context)) return context;
  const independent = new Set(["label", "name", "enabled", "priority", "weight", "supported_models", "model_selection", "models", "model_aliases"]);
  return Object.fromEntries(Object.entries(context).filter(([key]) => !independent.has(key)));
}

function localBinding(document: ConsoleRouteDocument, path: string): unknown {
  const context = ownerContext(document, path);
  if (!isRecord(context) || !Object.hasOwn(context, "settings")) return secretBinding(context);
  if (Object.hasOwn(context, "account")) return { settings: secretBinding(context.settings), account: secretBinding(context.account) };
  // Provider secrets can be inherited by accounts, so their destinations belong to the same binding.
  const accounts = Array.isArray(context.accounts) ? context.accounts.map(secretBinding) : context.accounts;
  return { settings: secretBinding(context.settings), accounts };
}

/** Rebuild secret operations by stable IDs; never read credential values during conflict recovery. */
export function rebaseRouteDraft(base: Config, draft: ConsoleRouteConfigCommitRequest, latest: Config): ConsoleRouteConfigCommitRequest {
  if (base.revision.id !== draft.expectedRevision || !latest.mutationSupported) conflict();
  const document = mergeRouteValue(base.document, draft.document, latest.document) as ConsoleRouteDocument;
  const explicit = draft.secretPatches.filter(patch => patch.operation !== "keep");
  for (const patch of explicit) {
    // Nested arrays have no stable client-side identity; do not guess which secret was edited.
    if (crossesNestedArray(draft.document, patch.path)) conflict();
    const oldPath = remap(draft.document, base.document, [patch])[0]?.path;
    const newPath = remap(draft.document, latest.document, [patch])[0]?.path;
    if (oldPath && crossesNestedArray(base.document, oldPath) || newPath && crossesNestedArray(latest.document, newPath)) conflict();
    // New owners can be added locally; an existing owner must still have the same audience.
    if (!oldPath && !newPath) continue;
    if (!oldPath || !newPath || !sameRouteValue(ownerContext(base.document, oldPath), ownerContext(latest.document, newPath))) conflict();
    const oldSecret = base.secrets.find(secret => secret.path === oldPath);
    const newSecret = latest.secrets.find(secret => secret.path === newPath);
    if (oldSecret?.configured && !oldSecret.fingerprint || newSecret?.configured && !newSecret.fingerprint
      || !sameRouteValue(fieldIdentity(oldSecret), fieldIdentity(newSecret))) conflict();
  }
  // A newly rotated secret must not be deleted or rebound to a locally changed destination.
  for (const secret of latest.secrets) {
    if (!secret.configured) continue;
    const keep = { path: secret.path, operation: "keep" as const };
    const targetPath = remap(latest.document, document, [keep])[0]?.path;
    const oldPath = remap(latest.document, base.document, [keep])[0]?.path;
    const oldSecret = base.secrets.find(entry => entry.path === oldPath);
    const unchanged = sameRouteValue(fieldIdentity(oldSecret), fieldIdentity(secret)) && Boolean(secret.fingerprint);
    if (!targetPath) { if (!unchanged) conflict(); continue; }
    const localPath = remap(latest.document, draft.document, [keep])[0]?.path;
    if (!unchanged && oldPath && localPath
      && !sameRouteValue(localBinding(base.document, oldPath), localBinding(draft.document, localPath))) conflict();
  }
  const secretPatches = new Map(buildCredentialSecretPatches({ activeDocument: latest.document,
    draftDocument: document, activeSecrets: latest.secrets }).map(patch => [patch.path, patch]));
  const mapped = remap(draft.document, document, explicit);
  if (mapped.length !== explicit.length) conflict();
  for (const patch of mapped) secretPatches.set(patch.path, patch);
  return { ...draft, document, expectedRevision: latest.revision.id, secretPatches: [...secretPatches.values()] };
}
