import { expect, it } from "vitest";
import type { ConsoleRouteConfigCommitRequest, ConsoleRouteDocument } from "../../api/contracts";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { rebaseRouteDraft } from "./rebaseRouteDraft";

async function fixture() {
  const { routeConfig: base } = await createConsoleApi().getRouteConfig("test");
  base.document = { providers: [{ id: "p", label: "Before", base_url: "https://old.example/v1",
    credentials: [{ id: "a", expires_at: 10 }, { id: "b", expires_at: 20 }] }], model_routes: [], aliases: {} };
  base.secrets = ["a", "b"].map((id, index) => ({ path: `/providers/0/credentials/${index}/api_key`, configured: true, fingerprint: id }));
  const latest = structuredClone(base); latest.revision.id = "r2";
  const draft: ConsoleRouteConfigCommitRequest = { expectedRevision: base.revision.id, document: structuredClone(base.document),
    secretPatches: base.secrets.map(secret => ({ path: secret.path, operation: "keep" })) };
  return { base, latest, draft };
}

const provider = (document: ConsoleRouteDocument) => document.providers[0] as Record<string, unknown>;
const accounts = (document: ConsoleRouteDocument) => provider(document).credentials as Record<string, unknown>[];

it("keeps credential identity across remote provider/account reordering and new secrets", async () => {
  const { base, latest, draft } = await fixture();
  provider(draft.document).label = "After";
  draft.secretPatches[0] = { path: base.secrets[0].path, operation: "replace", value: "replacement-a" };
  accounts(latest.document).reverse();
  latest.document.providers.unshift({ id: "new", credentials: [{ id: "new-account" }] });
  latest.secrets = [
    { path: "/providers/0/credentials/0/api_key", configured: true, fingerprint: "new" },
    { ...base.secrets[1], path: "/providers/1/credentials/0/api_key" },
    { ...base.secrets[0], path: "/providers/1/credentials/1/api_key" },
  ];
  const result = rebaseRouteDraft(base, draft, latest);
  expect(result.document.providers[1]).toMatchObject({ id: "p", label: "After", credentials: [{ id: "b" }, { id: "a" }] });
  expect(result.secretPatches).toEqual([
    { path: "/providers/0/credentials/0/api_key", operation: "keep" },
    { path: "/providers/1/credentials/0/api_key", operation: "keep" },
    { path: "/providers/1/credentials/1/api_key", operation: "replace", value: "replacement-a" },
  ]);
});

it("does not delete an owner whose secret rotated while the editor was open", async () => {
  const { base, latest, draft } = await fixture();
  accounts(draft.document).pop(); latest.secrets[1].fingerprint = "rotated-b";
  expect(() => rebaseRouteDraft(base, draft, latest)).toThrow(/冲突/);
});

it("does not replay a staged key onto a remotely changed target", async () => {
  const { base, latest, draft } = await fixture();
  provider(latest.document).base_url = "https://remote.example/v1";
  draft.secretPatches[0] = { path: base.secrets[0].path, operation: "replace", value: "replacement-a" };
  expect(() => rebaseRouteDraft(base, draft, latest)).toThrow(/冲突/);
});

it.each(["base_url", "auth_mode"])("does not keep a newly rotated key with a locally changed %s", async field => {
  const { base, latest, draft } = await fixture();
  provider(draft.document)[field] = field === "base_url" ? "https://new.example/v1" : "session";
  latest.secrets[0].fingerprint = "rotated-a";
  expect(() => rebaseRouteDraft(base, draft, latest)).toThrow(/冲突/);
});

it("allows local name and model changes alongside a remote token refresh", async () => {
  const { base, latest, draft } = await fixture();
  provider(draft.document).label = "After"; accounts(draft.document)[0].supported_models = ["text-model"];
  accounts(latest.document)[0].expires_at = 30; latest.secrets[0].fingerprint = "rotated-a";
  const result = rebaseRouteDraft(base, draft, latest);
  expect(result.document.providers[0]).toMatchObject({ label: "After", credentials: [{ id: "a", expires_at: 30, supported_models: ["text-model"] }, { id: "b" }] });
});

it("preserves keys staged for a newly added owner", async () => {
  const { base, latest, draft } = await fixture();
  draft.document.providers.push({ id: "new", credentials: [{ id: "new-account" }] });
  draft.secretPatches.push({ path: "/providers/1/credentials/0/api_key", operation: "replace", value: "new-key" });
  latest.document.aliases = { remote: "model" };
  const result = rebaseRouteDraft(base, draft, latest);
  expect(result.document.aliases).toEqual({ remote: "model" });
  expect(result.secretPatches.at(-1)).toEqual(draft.secretPatches.at(-1));
});

it.each(["keep", "replace", "clear"] as const)("protects inherited account targets when rebasing a shared provider secret (%s)", async operation => {
  const { base, latest, draft } = await fixture();
  base.secrets = [{ path: "/providers/0/api_key", configured: true, fingerprint: "provider-key" }];
  latest.secrets = structuredClone(base.secrets);
  draft.secretPatches = [{ path: "/providers/0/api_key", operation, ...(operation === "replace" ? { value: "new-provider-key" } : {}) }];
  if (operation === "keep") {
    latest.secrets[0].fingerprint = "rotated-provider-key";
    accounts(draft.document)[0].base_url = "https://new-local.example/v1";
  } else accounts(latest.document)[0].base_url = "https://new-remote.example/v1";
  expect(() => rebaseRouteDraft(base, draft, latest)).toThrow(/冲突/);
});

it("allows display and model edits for accounts inheriting a rotated provider secret", async () => {
  const { base, latest, draft } = await fixture();
  base.secrets = [{ path: "/providers/0/api_key", configured: true, fingerprint: "before" }];
  latest.secrets = [{ ...base.secrets[0], fingerprint: "after" }];
  draft.secretPatches = [{ path: "/providers/0/api_key", operation: "keep" }];
  accounts(draft.document)[0].label = "Account name";
  accounts(draft.document)[0].supported_models = ["text-model"];
  expect(rebaseRouteDraft(base, draft, latest).document.providers[0]).toMatchObject({
    credentials: [{ id: "a", label: "Account name", supported_models: ["text-model"] }, { id: "b" }],
  });
});

it.each(["replace", "clear"] as const)("refuses %s across nested arrays instead of comparing another item's secret", async operation => {
  const { base, latest, draft } = await fixture();
  const array = [{ name: "A", token: null }, { name: "B", token: null }];
  provider(base.document).extra_body = { items: structuredClone(array) };
  provider(latest.document).extra_body = { items: structuredClone(array) };
  provider(draft.document).extra_body = { items: [...array].reverse() };
  base.secrets.push(...["A", "B"].map((fingerprint, index) => ({ path: `/providers/0/extra_body/items/${index}/token`, configured: true, fingerprint })));
  latest.secrets = structuredClone(base.secrets); latest.secrets.at(-1)!.fingerprint = "rotated-B";
  draft.secretPatches.push({ path: "/providers/0/extra_body/items/0/token", operation, ...(operation === "replace" ? { value: "replacement-B" } : {}) });
  expect(() => rebaseRouteDraft(base, draft, latest)).toThrow(/冲突/);
});
