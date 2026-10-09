import { expect, it } from "vitest";
import type { ConsoleRouteDocument, CredentialTestScope } from "../../api/contracts";
import { defaultTestPolicy, updateTestPolicy } from "./credentialTestPolicyDocument";
import { selectedTestAccounts, selectedTestCallCount, testInvocations } from "./credentialTestSelection";

const scopes: CredentialTestScope[] = [{ kind: "pool", providerId: "p" }, { kind: "pool", providerId: "q" }];
const document: ConsoleRouteDocument = { providers: scopes.map((scope) => ({ id: scope.providerId,
  supported_models: ["a"], credentials: [{ id: "same-id", api_key: "fixture" }],
})), model_routes: [], aliases: {}, account_groups: [] };

it("keeps equal account IDs from different providers separate", () => {
  const accounts = selectedTestAccounts(document, scopes);
  expect(accounts[0].key).not.toBe(accounts[1].key);
  expect(testInvocations(document, scopes, defaultTestPolicy(), [accounts[1].key])).toEqual([
    { providerId: "q", request: { scope: { kind: "pool" }, testPlan: defaultTestPolicy(), credentialIds: ["same-id"] } },
  ]);
});

it("counts lower-level overrides in the global call bound", () => {
  const copy = structuredClone(document);
  Object.assign(copy.providers[1] as Record<string, unknown>, { default_model: "a", credentials: [{ id: "same-id", test_policy: {
    ...defaultTestPolicy(), modelSelection: "selected", models: Array.from({ length: 128 }, (_, index) => `m${index}`),
  } }] });
  const selected = selectedTestAccounts(copy, scopes).map((account) => account.key);
  expect(selectedTestCallCount(copy, scopes, defaultTestPolicy(), selected)).toBe(129);
  expect(selectedTestCallCount(copy, [scopes[0]], defaultTestPolicy(), selected.slice(0, 1))).toBe(1);
});

it("preserves different saved target policies during a mixed manual round", () => {
  const first = { ...defaultTestPolicy(), intervalMinutes: 7, models: ["first"], modelSelection: "selected" as const };
  const second = { ...first, intervalMinutes: 9, models: ["second"] };
  const configured = updateTestPolicy(updateTestPolicy(document, scopes[0], first), scopes[1], second);
  const selected = selectedTestAccounts(configured, scopes).map((account) => account.key);
  const requests = testInvocations(configured, scopes, first, selected, { mixed: true });
  expect(requests.map((entry) => entry.request.testPlan?.models)).toEqual([["first"], ["second"]]);
});
