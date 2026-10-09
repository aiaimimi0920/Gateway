import { expect, it } from "vitest";
import type { ConsoleRouteDocument, CredentialTestScope } from "../../api/contracts";
import { defaultTestPolicy, parentTestPolicy, readTestPolicy, testProviderContext, updateTestPolicy, updateTestPolicies } from "./credentialTestPolicyDocument";

const pool: CredentialTestScope = { kind: "pool", providerId: "p" };
const group: CredentialTestScope = { kind: "subpool", providerId: "p", id: "free" };
const account: CredentialTestScope = { kind: "account", providerId: "p", id: "p-cred-0" };
const document: ConsoleRouteDocument = { providers: [{ id: "p", supported_models: ["a"], default_model: "not-selected",
  model_map: { alias: "a" }, scheduled_probe_enabled: true, scheduled_probe_interval_minutes: 15,
  credential_identity_categories: [{ id: "free", label: "Free" }],
  credentials: [{ api_key: "preserved-secret", credential_identity_category_id: "free" }],
}], model_routes: [], aliases: {}, account_groups: [] };

it("inherits legacy pool settings consistently and uses runtime-exact model candidates", () => {
  for (const scope of [pool, group, account]) expect(readTestPolicy(document, scope).policy).toMatchObject({ automaticEnabled: true, intervalMinutes: 15 });
  expect(testProviderContext(document, "p")?.models).toEqual(["a"]);
  expect(testProviderContext(document, "p")?.accounts[0].id).toBe("p-cred-0");
});

it("resolves three levels and changing a parent preserves children, IDs and secrets", () => {
  const poolPolicy = { ...defaultTestPolicy(), models: ["pool"], modelSelection: "selected" as const };
  const groupPolicy = { ...poolPolicy, models: ["group"] };
  const accountPolicy = { ...poolPolicy, models: ["account"], automaticEnabled: false };
  let next = updateTestPolicy(document, pool, poolPolicy);
  next = updateTestPolicy(next, group, groupPolicy);
  next = updateTestPolicy(next, account, accountPolicy);
  expect(readTestPolicy(next, account).policy.models).toEqual(["account"]);
  expect(parentTestPolicy(next, account).models).toEqual(["group"]);
  next = updateTestPolicy(next, pool, { ...poolPolicy, models: ["new-pool"] });
  expect(readTestPolicy(next, account).policy).toEqual(accountPolicy);
  expect(readTestPolicy(next, group).policy).toEqual(groupPolicy);
  expect(next.providers[0]).toMatchObject({ credentials: [{ api_key: "preserved-secret" }] });
  expect(document.providers[0]).not.toHaveProperty("test_policy");
  next = updateTestPolicy(next, account, null);
  expect(readTestPolicy(next, account)).toMatchObject({ own: false, policy: groupPolicy });
});

it("saves a default account override separately and rejects foreign owners", () => {
  const single = { ...document, providers: [{ id: "p", api_key: "secret", supported_models: ["a"] }] };
  const next = updateTestPolicy(single, { kind: "account", providerId: "p", id: "p::default" }, defaultTestPolicy());
  expect(next.providers[0]).toHaveProperty("default_account_test_policy");
  expect(next.providers[0]).not.toHaveProperty("test_policy");
  expect(() => updateTestPolicy(single, account, defaultTestPolicy())).toThrow("Test account was not found");
});

it("saves multiple subpools atomically while all three policy levels coexist", () => {
  const second: CredentialTestScope = { kind: "subpool", providerId: "p", id: "plus" };
  const source = structuredClone(document);
  Object.assign(source.providers[0] as Record<string, unknown>, { credential_identity_categories: [{ id: "free", label: "Free" }, { id: "plus", label: "Plus" }, { id: "pro", label: "Pro" }] });
  let next = updateTestPolicy(source, pool, { ...defaultTestPolicy(), intervalMinutes: 20 });
  next = updateTestPolicy(next, account, { ...defaultTestPolicy(), intervalMinutes: 3 });
  next = updateTestPolicy(next, { ...second, id: "pro" }, { ...defaultTestPolicy(), intervalMinutes: 99 });
  next = updateTestPolicies(next, [group, second], { ...defaultTestPolicy(), intervalMinutes: 7 });
  expect(readTestPolicy(next, pool).policy.intervalMinutes).toBe(20);
  expect(readTestPolicy(next, account).policy.intervalMinutes).toBe(3);
  expect(readTestPolicy(next, group).policy.intervalMinutes).toBe(7);
  expect(readTestPolicy(next, second).policy.intervalMinutes).toBe(7);
  expect(readTestPolicy(next, { ...second, id: "pro" }).policy.intervalMinutes).toBe(99);
  expect(next.providers[0]).toMatchObject({ credentials: [{ api_key: "preserved-secret" }] });
  const before = structuredClone(next);
  expect(() => updateTestPolicies(next, [group, { ...second, id: "missing" }], defaultTestPolicy())).toThrow();
  expect(next).toEqual(before);
  expect(() => updateTestPolicies(next, [group, group], defaultTestPolicy())).toThrow();
});
