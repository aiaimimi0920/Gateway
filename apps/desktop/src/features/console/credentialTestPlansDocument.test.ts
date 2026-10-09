import { expect, it } from "vitest";
import type { ConsoleRouteDocument, CredentialTestPlan } from "../../api/contracts";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import { prepareTestPlanRun, testPlanRows, updateTestPlans } from "./credentialTestPlansDocument";

const document: ConsoleRouteDocument = { providers: [{ id: "p", api_key: "preserved", test_policy: defaultTestPolicy(),
  subpool_test_policies: { free: defaultTestPolicy() }, credentials: [{ id: "one", api_key: "account-secret", credential_identity_category_id: "free" }] }],
  model_routes: [], aliases: {}, account_groups: [] };
const plan = (id: string): CredentialTestPlan => ({ id, name: id, scopes: [{ kind: "pool" }], policy: defaultTestPolicy() });

it("saves two plans in one scope without touching secrets or the three legacy layers", () => {
  const next = updateTestPlans(document, [{ providerId: "p", id: "first", plan: plan("first") }, { providerId: "p", id: "second", plan: plan("second") }]);
  expect(testPlanRows(next, ["p"]).map((row) => row.name)).toEqual(["first", "second", "凭据池 · p", "子池 · p / free"]);
  expect(next.providers[0]).toMatchObject({ api_key: "preserved", test_policy: defaultTestPolicy(),
    credentials: [{ id: "one", api_key: "account-secret" }] });
  expect(document.providers[0]).not.toHaveProperty("test_plans");
  const renamed = updateTestPlans(next, [{ providerId: "p", id: "first", plan: { ...plan("first"), name: "renamed" } }]);
  const removed = updateTestPlans(renamed, [{ providerId: "p", id: "second", plan: null }]);
  expect(testPlanRows(removed, ["p"]).filter((row) => !row.legacy).map((row) => [row.id, row.name])).toEqual([["first", "renamed"]]);
});

it("rejects invalid scopes, names, duplicate batches and missing owners atomically", () => {
  for (const bad of [ { ...plan("bad"), name: " " }, { ...plan("bad"), scopes: [{ kind: "account" as const, id: "foreign" }] },
    { ...plan("bad"), scopes: [{ kind: "pool" as const }, { kind: "subpool" as const, id: "free" }] } ]) {
    expect(() => updateTestPlans(document, [{ providerId: "p", id: "first", plan: plan("first") }, { providerId: "p", id: "bad", plan: bad }])).toThrow();
  }
  const change = { providerId: "p", id: "first", plan: plan("first") };
  expect(() => updateTestPlans(document, [change, change])).toThrow();
  expect(() => updateTestPlans(document, [change, { ...change, providerId: "foreign" }])).toThrow();
  expect(() => updateTestPlans(document, [{ providerId: "p", id: "missing", plan: null }])).toThrow();
  expect(document.providers[0]).not.toHaveProperty("test_plans");
});

it("does not pass incomplete raw JSON plans into the editor", () => {
  const draft = structuredClone(document);
  const provider = draft.providers[0] as Record<string, unknown>;
  provider.test_plans = [plan("valid"), { ...plan("missing-policy"), policy: {} },
    { ...plan("null-case"), policy: { ...defaultTestPolicy(), cases: [null] } },
    { ...plan("null-scope"), scopes: [null] }];
  expect(testPlanRows(draft, ["p"]).filter((row) => !row.legacy).map((row) => row.id)).toEqual(["valid"]);
  expect(provider.test_plans).toHaveLength(4);
});

it("runs a saved plan over every enabled account in its scope without applying legacy overrides", () => {
  const source: ConsoleRouteDocument = { ...document, providers: [{ id: "p", supported_models: ["a"],
    credentials: [{ id: "one", credential_identity_category_id: "free", test_policy: defaultTestPolicy() },
      { id: "disabled", enabled: false, credential_identity_category_id: "free" }, { id: "paid", credential_identity_category_id: "plus" }],
    test_plans: [{ ...plan("named"), scopes: [{ kind: "subpool", id: "free" }] }],
  }] };
  const row = testPlanRows(source, ["p"])[0];
  expect(prepareTestPlanRun(source, row)).toEqual({ error: null,
    invocations: [{ providerId: "p", request: { planId: "named", credentialIds: ["one"] } }] });
  expect(prepareTestPlanRun(source, { ...row, scopes: [{ kind: "subpool", providerId: "p", id: "missing" }] }).error).toMatch(/暂无启用/);
});

it("preserves legacy execution payloads and refuses the complete round above the call budget", () => {
  const source = structuredClone(document);
  const provider = source.providers[0] as Record<string, unknown>;
  provider.supported_models = ["a"];
  const legacy = testPlanRows(source, ["p"]).find((row) => row.legacy && row.scopes[0].kind === "pool")!;
  expect(prepareTestPlanRun(source, legacy)).toMatchObject({ error: null, invocations: [{ providerId: "p",
    request: { scope: { kind: "pool" }, credentialIds: ["one"], testPlan: defaultTestPolicy() } }] });
  provider.credentials = Array.from({ length: 129 }, (_, index) => ({ id: String(index) }));
  expect(prepareTestPlanRun(source, legacy).error).toMatch(/128/);
  provider.credentials = [{ id: "one" }];
  provider.supported_models = Array.from({ length: 129 }, (_, index) => String(index));
  const named = { ...legacy, id: "named", legacy: false };
  expect(prepareTestPlanRun(source, named).error).toMatch(/128/);
});
