import type { ConsoleProviderProbeRequest, ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import { parentTestPolicy, readTestPolicy, testProviderContext, testScopeKey } from "./credentialTestPolicyDocument";

export type TestSelectionAccount = {
  key: string; providerId: string; id: string; name: string; models: string[]; enabled: boolean;
};
export type TestInvocation = { providerId: string; request: ConsoleProviderProbeRequest };
type SelectionPolicy = { inherit?: boolean; mixed?: boolean };

function targetPolicy(document: ConsoleRouteDocument | null, scope: CredentialTestScope, draft: CredentialTestPolicy, selection: SelectionPolicy) {
  return selection.inherit ? parentTestPolicy(document, scope) : selection.mixed ? readTestPolicy(document, scope).policy : draft;
}

export function selectedTestAccounts(document: ConsoleRouteDocument | null, scopes: CredentialTestScope[]) {
  return scopes.flatMap((scope) => (testProviderContext(document, scope.providerId)?.accounts ?? [])
    .filter((account) => scope.kind === "pool" || (scope.kind === "account" ? account.id === scope.id : account.category === scope.id))
    .map((account): TestSelectionAccount => ({ ...account, providerId: scope.providerId,
      key: testScopeKey({ kind: "account", providerId: scope.providerId, id: account.id }) })));
}

export function testInvocations(document: ConsoleRouteDocument | null, scopes: CredentialTestScope[],
  policy: CredentialTestPolicy, selected: string[], selection: SelectionPolicy = {}): TestInvocation[] {
  return scopes.map((scope) => ({ providerId: scope.providerId, request: {
    scope: scope.kind === "pool" ? { kind: "pool" as const } : { kind: scope.kind, id: scope.id },
    testPlan: targetPolicy(document, scope, policy, selection),
    credentialIds: selectedTestAccounts(document, [scope]).filter((account) => account.enabled && selected.includes(account.key)).map((account) => account.id),
  } })).filter((entry) => entry.request.credentialIds!.length > 0);
}

// Bound the whole UI round, including saved lower-level overrides, before sending any request.
export function selectedTestCallCount(document: ConsoleRouteDocument | null, scopes: CredentialTestScope[],
  draft: CredentialTestPolicy, selected: string[], selection: SelectionPolicy = {}) {
  if (selected.length > 128) return 129;
  return scopes.reduce((total, scope) => total + selectedTestAccounts(document, [scope])
    .filter((account) => account.enabled && selected.includes(account.key)).reduce((count, account) => {
      const accountScope: CredentialTestScope = { kind: "account", providerId: scope.providerId, id: account.id };
      const savedAccount = readTestPolicy(document, accountScope);
      const category = testProviderContext(document, scope.providerId)?.accounts.find((entry) => entry.id === account.id)?.category;
      const savedSubpool = category ? readTestPolicy(document, { kind: "subpool", providerId: scope.providerId, id: category }) : null;
      const policy = scope.kind !== "account" && savedAccount.own ? savedAccount.policy
        : scope.kind === "pool" && savedSubpool?.own ? savedSubpool.policy : targetPolicy(document, scope, draft, selection);
      return count + (policy.modelSelection === "all" ? account.models.length : policy.models.length)
        * policy.cases.filter((entry) => entry.enabled).length;
    }, 0), 0);
}
