// Pure route-document ownership: parent saves never rewrite child overrides or credentials.
import type { ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import { readPilotIdentityCategories } from "./pilotPoolPolicy";
import { iqExtraCases } from "./credentialIqBank";

export type TestScopeOption = { key: string; label: string; scope: CredentialTestScope };
export type TestAccount = { id: string; name: string; category: string | null; models: string[]; enabled: boolean };
export const testScopeKey = (scope: CredentialTestScope) => JSON.stringify([scope.providerId, scope.kind, "id" in scope ? scope.id : ""]);

export function defaultTestPolicy(): CredentialTestPolicy {
  return { automaticEnabled: false, intervalMinutes: 60, modelSelection: "all", models: [], cases: [
    { id: "connection", name: "连通与指令遵循", prompt: "Reply with only OK.", expectedAnswer: "OK", difficulty: 1, enabled: true },
    { id: "arithmetic", name: "计算能力", prompt: "What is 17 * 23? Reply with only the number.", expectedAnswer: "391", difficulty: 2, enabled: false },
    { id: "reasoning", name: "多步推理", prompt: "A counter starts at 120, subtracts 35, adds 19, then divides by 2. What is the final value? Reply with only the number.", expectedAnswer: "52", difficulty: 3, enabled: false },
    ...iqExtraCases.map((item) => ({ ...item })),
  ] };
}

function stringModels(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((model): model is string => typeof model === "string" && model.length > 0
    && model.trim() === model && new TextEncoder().encode(model).length <= 256 && !/[*?\u0000-\u001f\u007f-\u009f]/u.test(model)) : [];
}
function policy(value: unknown): CredentialTestPolicy | null {
  return isRecord(value) && Array.isArray(value.cases) && Array.isArray(value.models)
    ? structuredClone(value) as CredentialTestPolicy : null;
}

export function testProviderContext(document: ConsoleRouteDocument | null, providerId: string) {
  const provider = document?.providers.find((entry) => isRecord(entry) && entry.id === providerId);
  if (!isRecord(provider)) return null;
  const label = typeof provider.label === "string" ? provider.label : providerId;
  const credentials = Array.isArray(provider.credentials) ? provider.credentials.filter(isRecord) : [];
  const declared = stringModels(provider.supported_models);
  const poolModels = declared.length ? declared : stringModels([provider.default_model]);
  const accounts: TestAccount[] = credentials.length ? credentials.map((credential, index) => ({
    id: String(credential.id ?? `${providerId}-cred-${index}`), name: String(credential.account_name ?? credential.id ?? `${providerId}-cred-${index}`),
    category: typeof credential.credential_identity_category_id === "string" ? credential.credential_identity_category_id : null,
    models: stringModels(credential.supported_models).length ? stringModels(credential.supported_models) : poolModels,
    enabled: credential.enabled !== false,
  })) : [{ id: `${providerId}::default`, name: label, category: null, models: poolModels, enabled: true }];
  const categories = readPilotIdentityCategories(provider, providerId, label);
  const options: TestScopeOption[] = [
    { scope: { kind: "pool" as const, providerId }, label: `凭据池 · ${label}`, key: "" },
    ...categories.map((category) => ({ scope: { kind: "subpool" as const, providerId, id: category.id }, label: `子池 · ${label} / ${category.label}`, key: "" })),
    ...accounts.map((account) => ({ scope: { kind: "account" as const, providerId, id: account.id }, label: `账户 · ${account.name}`, key: "" })),
  ].map((option) => ({ ...option, key: testScopeKey(option.scope) }));
  return { provider, credentials, accounts, options, models: [...new Set([...poolModels, ...accounts.flatMap((account) => account.models)])] };
}

export function readTestPolicy(document: ConsoleRouteDocument | null, scope: CredentialTestScope) {
  const context = testProviderContext(document, scope.providerId);
  const fallback = defaultTestPolicy();
  if (!context) return { policy: fallback, own: false, source: "默认策略" };
  const { provider, credentials, accounts } = context;
  const pool = policy(provider.test_policy);
  fallback.automaticEnabled = provider.scheduled_probe_enabled === true;
  fallback.intervalMinutes = typeof provider.scheduled_probe_interval_minutes === "number" ? provider.scheduled_probe_interval_minutes : 60;
  const subpools = isRecord(provider.subpool_test_policies) ? provider.subpool_test_policies : {};
  if (scope.kind === "pool") {
    return { policy: pool ?? fallback, own: !!pool, source: pool ? "凭据池策略" : "兼容默认策略" };
  }
  const account = scope.kind === "account" ? accounts.find((account) => account.id === scope.id) : null;
  const category = scope.kind === "subpool" ? scope.id : account?.category;
  const group = category ? policy(subpools[category]) : null;
  const credential = scope.kind === "account" ? credentials[accounts.findIndex((account) => account.id === scope.id)] : null;
  const own = scope.kind === "subpool" ? group : policy(credential ? credential.test_policy : provider.default_account_test_policy);
  return { policy: own ?? group ?? pool ?? fallback, own: !!own, source: own ? (scope.kind === "account" ? "账户独立策略" : "子池独立策略") : group ? "继承子池策略" : "继承凭据池策略" };
}

export function updateTestPolicy(document: ConsoleRouteDocument, scope: CredentialTestScope, nextPolicy: CredentialTestPolicy | null): ConsoleRouteDocument {
  return updateTestPolicies(document, [scope], nextPolicy);
}

// Apply a selection atomically; a missing owner never leaves a partially updated draft.
export function updateTestPolicies(document: ConsoleRouteDocument, scopes: CredentialTestScope[], nextPolicy: CredentialTestPolicy | null): ConsoleRouteDocument {
  if (!scopes.length || scopes.length > 128 || new Set(scopes.map(testScopeKey)).size !== scopes.length) throw new Error("Select 1–128 distinct policy targets.");
  const next = structuredClone(document);
  for (const scope of scopes) assignTestPolicy(next, scope, nextPolicy);
  return next;
}

function assignTestPolicy(next: ConsoleRouteDocument, scope: CredentialTestScope, nextPolicy: CredentialTestPolicy | null) {
  const context = testProviderContext(next, scope.providerId);
  if (!context) throw new Error("Test policy provider was not found.");
  const { provider, credentials } = context;
  let owner = provider;
  let field = "test_policy";
  if (scope.kind === "subpool") {
    if (!context.options.some((option) => testScopeKey(option.scope) === testScopeKey(scope))) throw new Error("Test subpool was not found.");
    const groups = isRecord(provider.subpool_test_policies) ? provider.subpool_test_policies : {};
    provider.subpool_test_policies = groups;
    owner = groups; field = scope.id;
  } else if (scope.kind === "account") {
    const credential = credentials[context.accounts.findIndex((account) => account.id === scope.id)];
    if (credential) owner = credential;
    else if (credentials.length === 0 && scope.id === `${scope.providerId}::default`) field = "default_account_test_policy";
    else throw new Error("Test account was not found in this pool.");
  }
  if (nextPolicy) owner[field] = structuredClone(nextPolicy); else delete owner[field];
}

export function parentTestPolicy(document: ConsoleRouteDocument | null, scope: CredentialTestScope): CredentialTestPolicy {
  return document ? readTestPolicy(updateTestPolicy(document, scope, null), scope).policy : defaultTestPolicy();
}

export function testPolicyError(plan: CredentialTestPolicy): string | null {
  const bytes = (text: string) => new TextEncoder().encode(text).length;
  const exact = (text: string) => text.length > 0 && text.trim() === text && bytes(text) <= 256 && !/[*?\u0000-\u001f\u007f-\u009f]/u.test(text);
  if (!Number.isInteger(plan.intervalMinutes) || plan.intervalMinutes < 1 || plan.intervalMinutes > 10_080) return "间隔须为 1–10080 分钟的整数。";
  if (plan.models.length > 128 || plan.models.some((model) => !exact(model)) || new Set(plan.models).size !== plan.models.length || (plan.modelSelection === "selected" && plan.models.length === 0)) return "请选择全部模型或至少一个精确模型；最多 128 个，不接受通配符。";
  if (plan.cases.length > 16 || plan.cases.length === 0 || !plan.cases.some((item) => item.enabled) || new Set(plan.cases.map((item) => item.id)).size !== plan.cases.length) return "请选择至少一个测试用例；最多保留 16 个不同用例。";
  if (plan.cases.some((item) => !exact(item.id) || !item.name.trim() || bytes(item.name) > 256 || !item.prompt.trim() || bytes(item.prompt) > 8192 || ![1, 2, 3].includes(item.difficulty) || (item.expectedAnswer != null && (!item.expectedAnswer.trim() || bytes(item.expectedAnswer) > 4096)))) return "用例提示词须为 1–8192 字节，标准答案为 1–4096 字节，难度为 1–3。";
  if (plan.modelSelection === "selected" && plan.models.length * plan.cases.filter((item) => item.enabled).length > 128) return "每账户一轮最多 128 个模型×用例组合。";
  return null;
}
