// Named collections are additive; mutations clone once and never rewrite credentials or old policies.
import type { ConsoleRouteDocument, CredentialTestPlan, CredentialTestScope } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { readTestPolicy, testPolicyError, testProviderContext, testScopeKey } from "./credentialTestPolicyDocument";
import { selectedTestAccounts, selectedTestCallCount, testInvocations } from "./credentialTestSelection";

export type TestPlanChange = { providerId: string; id: string; plan: CredentialTestPlan | null };
export type TestPlanRow = { key: string; providerId: string; id: string; name: string; scopes: CredentialTestScope[];
  policy: CredentialTestPlan["policy"]; legacy: boolean };

// JSON drafts can be incomplete before server validation; only pass safe shapes to the editor.
function editablePlan(value: unknown): value is CredentialTestPlan {
  if (!isRecord(value) || typeof value.id !== "string" || typeof value.name !== "string"
    || !Array.isArray(value.scopes) || !isRecord(value.policy)) return false;
  const policy = value.policy;
  return value.scopes.length > 0 && value.scopes.every((scope) => isRecord(scope)
    && (scope.kind === "pool" || ((scope.kind === "account" || scope.kind === "subpool") && typeof scope.id === "string")))
    && typeof policy.automaticEnabled === "boolean" && typeof policy.intervalMinutes === "number"
    && (policy.modelSelection === "all" || policy.modelSelection === "selected")
    && Array.isArray(policy.models) && policy.models.every((model) => typeof model === "string")
    && Array.isArray(policy.cases) && policy.cases.every((item) => isRecord(item)
      && typeof item.id === "string" && typeof item.name === "string" && typeof item.prompt === "string"
      && typeof item.difficulty === "number" && typeof item.enabled === "boolean"
      && (item.expectedAnswer == null || typeof item.expectedAnswer === "string"));
}

export function testPlanRows(document: ConsoleRouteDocument | null, providerIds: string[]): TestPlanRow[] {
  return providerIds.flatMap((providerId) => {
    const context = testProviderContext(document, providerId);
    if (!context) return [];
    const named: TestPlanRow[] = (Array.isArray(context.provider.test_plans) ? context.provider.test_plans : [])
      .filter(editablePlan)
      .map((plan) => ({ ...plan, key: JSON.stringify([providerId, "plan", plan.id]), providerId, legacy: false,
        scopes: plan.scopes.map((scope) => ({ ...scope, providerId })) }));
    const extraSubpools = Object.keys(isRecord(context.provider.subpool_test_policies) ? context.provider.subpool_test_policies : {})
      .map((id) => ({ scope: { kind: "subpool" as const, providerId, id }, label: `子池 · ${providerId} / ${id}`, key: testScopeKey({ kind: "subpool", providerId, id }) }))
      .filter((option) => !context.options.some((existing) => existing.key === option.key));
    const legacy = [...context.options, ...extraSubpools].flatMap(({ scope, label, key }) => {
      const saved = readTestPolicy(document, scope);
      return saved.own ? [{ key, providerId, id: key, name: label, scopes: [scope], policy: saved.policy, legacy: true }] : [];
    });
    return [...named, ...legacy];
  });
}

export function namedPlanError(document: ConsoleRouteDocument, change: TestPlanChange): string | null {
  const context = testProviderContext(document, change.providerId);
  if (!context) return "测试计划的凭据池不存在。";
  if (!change.plan) return null;
  const plan = change.plan;
  const exact = (value: string) => value.length > 0 && value.trim() === value
    && new TextEncoder().encode(value).length <= 256 && !/[*?\p{Cc}]/u.test(value);
  if (plan.id !== change.id || !exact(plan.id) || !plan.name.trim() || /\p{Cc}/u.test(plan.name)
    || new TextEncoder().encode(plan.name).length > 256) return "请填写 1–256 字节的计划名称。";
  if (!plan.scopes.length || plan.scopes.length > 128 || new Set(plan.scopes.map((scope) => testScopeKey({ ...scope, providerId: change.providerId }))).size !== plan.scopes.length
    || plan.scopes.some((scope) => scope.kind !== plan.scopes[0].kind)
    || (plan.scopes.length > 1 && plan.scopes[0].kind === "pool")) return "请选择一个全池，或 1–128 个不同子池或账户。";
  if (plan.scopes.some((scope) => !context.options.some((option) => option.key === testScopeKey({ ...scope, providerId: change.providerId })))) return "计划范围不属于当前凭据池。";
  if (plan.scopes.some((scope) => scope.kind === "subpool" && !context.accounts.some((account) => account.category === scope.id))) return "所选子池暂无账户。";
  if (context.accounts.filter((account) => plan.scopes.some((scope) => scope.kind === "pool" || (scope.kind === "account" ? account.id === scope.id : account.category === scope.id))).length > 128) return "每个计划的范围最多包含 128 个账户。";
  return testPolicyError(plan.policy);
}

export function updateTestPlans(document: ConsoleRouteDocument, changes: TestPlanChange[]): ConsoleRouteDocument {
  if (!changes.length || changes.length > 128 || new Set(changes.map((change) => JSON.stringify([change.providerId, change.id]))).size !== changes.length) throw new Error("无效的测试计划变更。");
  const next = structuredClone(document);
  for (const change of changes) {
    const error = namedPlanError(next, change);
    if (error) throw new Error(error);
    const provider = testProviderContext(next, change.providerId)!.provider;
    const plans = Array.isArray(provider.test_plans) ? provider.test_plans as CredentialTestPlan[] : [];
    const existing = plans.findIndex((plan) => plan.id === change.id);
    if (!change.plan) {
      if (existing < 0) throw new Error("测试计划已不存在。请返回列表刷新。");
      plans.splice(existing, 1);
    } else if (existing < 0) plans.push(structuredClone(change.plan));
    else plans[existing] = structuredClone(change.plan);
    if (plans.length > 32 || new Set(plans.map((plan) => plan.id)).size !== plans.length) throw new Error("每个凭据池最多 32 个不同测试计划。");
    if (plans.length) provider.test_plans = plans;
    else delete provider.test_plans;
  }
  return next;
}

// Every list run covers enabled accounts in the saved scope, without a second account picker.
export function prepareTestPlanRun(document: ConsoleRouteDocument | null, row: TestPlanRow) {
  const accounts = selectedTestAccounts(document, row.scopes).filter((account) => account.enabled);
  const calls = row.legacy ? selectedTestCallCount(document, row.scopes, row.policy, accounts.map((account) => account.key))
    : accounts.reduce((total, account) => total + (row.policy.modelSelection === "all" ? account.models.length : row.policy.models.length)
      * row.policy.cases.filter((item) => item.enabled).length, 0);
  const error = testPolicyError(row.policy) ?? (!accounts.length ? "所选范围暂无启用的账户"
    : accounts.length > 128 || calls > 128 ? "本轮超过 128 次调用或账户" : null);
  const invocations = row.legacy ? testInvocations(document, row.scopes, row.policy, accounts.map((account) => account.key))
    : [{ providerId: row.providerId, request: { planId: row.id, credentialIds: accounts.map((account) => account.id) } }];
  return { error, invocations };
}
