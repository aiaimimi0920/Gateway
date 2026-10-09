// Each target selection owns its draft; changing mode never recreates it.
import { useMemo, useRef, useState } from "react";
import type { ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import type { AccountsLedgerPilotSection } from "./accountsLedgerTypes";
import { defaultTestPolicy, parentTestPolicy, readTestPolicy, testProviderContext, testScopeKey } from "./credentialTestPolicyDocument";
import { selectedTestAccounts, selectedTestCallCount, testInvocations } from "./credentialTestSelection";

type Selection = { kind: CredentialTestScope["kind"]; keys: string[] };
type Draft = { policy: CredentialTestPolicy; ownPolicy: CredentialTestPolicy | null; inherit: boolean; selected: string[]; mixed: boolean };
export function useCredentialTestDraft(document: ConsoleRouteDocument | null, section: AccountsLedgerPilotSection, initialScope?: CredentialTestScope, scopeLocked = false,
  plan?: { scopes: CredentialTestScope[]; policy: CredentialTestPolicy }) {
  const contexts = useMemo(() => section.providerIds.map((id) => testProviderContext(document, id)).filter((context) => context !== null), [document, section.providerIds]);
  const options = contexts.flatMap((context) => context.options);
  const poolKeys = options.filter((option) => option.scope.kind === "pool").map((option) => option.key);
  const [selection, setSelection] = useState<Selection>(() => ({ kind: plan?.scopes[0]?.kind ?? initialScope?.kind ?? "pool", keys: plan ? plan.scopes.map(testScopeKey) : initialScope ? [testScopeKey(initialScope)] : poolKeys }));
  const selections = useRef(new Map<Selection["kind"], Selection>());
  const targetsFor = (value: Selection) => options.filter((option) => value.keys.includes(option.key) && option.scope.kind === value.kind).map((option) => option.scope);
  const targets = scopeLocked && initialScope ? [initialScope] : targetsFor(selection);
  const keyFor = (value: Selection) => JSON.stringify([value.kind, [...value.keys].sort()]);
  const initialDraft = (scopes: CredentialTestScope[]): Draft => {
    const saved = scopes.map((scope) => readTestPolicy(document, scope));
    const policy = plan ? structuredClone(plan.policy) : saved[0]?.policy ?? defaultTestPolicy();
    const inherit = !plan && scopes.length > 0 && scopes.every((scope) => scope.kind !== "pool") && saved.every((value) => !value.own);
    return { policy, ownPolicy: inherit ? null : policy, inherit,
      selected: selectedTestAccounts(document, scopes).filter((account) => account.enabled).map((account) => account.key),
      mixed: !plan && saved.some((value) => value.own !== saved[0]?.own || JSON.stringify(value.policy) !== JSON.stringify(policy)) };
  };
  const [draft, setDraft] = useState<Draft>(() => initialDraft(targets));
  const drafts = useRef(new Map<string, Draft>());
  const changeSelection = (next: Selection) => {
    if (scopeLocked) return;
    drafts.current.set(keyFor(selection), structuredClone(draft));
    if (drafts.current.size > 128) drafts.current.delete(drafts.current.keys().next().value!);
    selections.current.set(selection.kind, selection);
    setSelection(next);
    // Named plans keep their edited policy when only their target membership changes.
    setDraft(plan ? { ...draft, selected: initialDraft(targetsFor(next)).selected }
      : drafts.current.get(keyFor(next)) ?? initialDraft(targetsFor(next)));
  };
  const accounts = selectedTestAccounts(document, targets).filter((account) => account.enabled);
  const allAccounts = selectedTestAccounts(document, targets);
  const configuredModels = [...new Set(allAccounts.flatMap((account) => account.models))];
  const models = [...new Set([...configuredModels, ...draft.policy.models])];
  const available = targets.length > 0 && targets.every((scope) => options.some((option) => option.key === testScopeKey(scope)));
  return { selection, targets, options, draft, accounts, allAccounts, models, configuredModels, scopeLocked, available,
    selectionKey: keyFor(selection), setDraft,
    changeKind: (kind: Selection["kind"]) => changeSelection(selections.current.get(kind) ?? { kind, keys: kind === "pool" ? poolKeys : [] }),
    changeKeys: (keys: string[]) => changeSelection({ ...selection, keys }),
    changeTargets: (kind: Selection["kind"], keys: string[]) => changeSelection({ kind, keys }),
    setInheritance: (inherit: boolean) => setDraft((current) => ({ ...current, inherit, mixed: false,
      ownPolicy: inherit ? current.policy : current.ownPolicy,
      policy: inherit && targets[0] ? parentTestPolicy(document, targets[0]) : current.ownPolicy ?? current.policy })),
    updatePolicy: (policy: CredentialTestPolicy) => setDraft((current) => ({ ...current, policy, mixed: false })),
    calls: plan ? accounts.filter((account) => draft.selected.includes(account.key)).reduce((total, account) => total
      + (draft.policy.modelSelection === "all" ? account.models.length : draft.policy.models.length) * draft.policy.cases.filter((item) => item.enabled).length, 0)
      : selectedTestCallCount(document, targets, draft.policy, draft.selected, draft),
    invocations: testInvocations(document, targets, draft.policy, draft.selected, draft) };
}
