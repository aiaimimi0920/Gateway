// A scoped Local Key may test one remote account in a shared Gateway without
// altering unrelated model routes. The reserved model never uses generic routing.
export function safeCursorErrorCode(value) {
  return typeof value === "string" && /^cursor_(?:remote|acp)_[a-z][a-z0-9_]{0,95}$/.test(value)
    ? value : null;
}

export function cursorErrorCodes(output) {
  // Match whole tokens before validating: never truncate http2 or an overlong code.
  return [...new Set((output.match(/\bcursor_(?:remote|acp)_[A-Za-z0-9_]+\b/g) ?? [])
    .map(safeCursorErrorCode).filter(Boolean))];
}

export function exactRoute(value, provider, credential) {
  const providers = value.document.providers.filter((p) => p.id === provider);
  if (providers.length !== 1 || providers[0].auth_mode !== "cursor_remote") {
    throw new Error("provider_mismatch");
  }
  const accounts = providers[0].credentials.filter((c) => c.enabled === true);
  if (accounts.length !== 1 || accounts[0].id !== credential || accounts[0].scheduled_probe_enabled ||
      (accounts[0].supported_models?.length ?? 0)) {
    throw new Error("exact_private_account_required");
  }
  return { revision: value.revision.id, provider, credential, enabled: true };
}

// A successful snapshot may omit fallbackEligible (it describes failures).
// Missing delivery evidence, in contrast, must never count as a confirmed send.
export function exactCompletedResponse(row, route) {
  const t = row?.routeTrace;
  return !!row?.completedAt && row.status === "completed" && row.endpointKind === "responses" &&
    row.upstreamStatus === 200 && row.providerAccountId === route.provider &&
    t?.routeAttemptCount === 1 && t.routeRevision === route.revision &&
    t.realCredentialRef === route.credential && t.selectedPipelineMode === "cursor_remote" &&
    t.cursorRemoteDeliveryUnknown === false && t.errorCode == null &&
    (t.fallbackEligible == null || t.fallbackEligible === false);
}
