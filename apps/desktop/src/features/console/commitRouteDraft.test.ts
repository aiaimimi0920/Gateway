import { expect, it, vi } from "vitest";
import { GatewayApiError } from "../../api/errors";
import type { ConsoleRouteConfigCommitRequest } from "../../api/contracts";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { commitRouteDraft } from "./commitRouteDraft";
import { deferred } from "./BrowserConsoleApp.render-fixture";

const conflict = () => new GatewayApiError("old revision", 409, "console_revision_conflict");
async function fixture() {
  const api = createConsoleApi();
  const base = await api.getRouteConfig("test");
  base.routeConfig.document = { providers: [{ id: "p", label: "Before", base_url: "https://example.test",
    credentials: [{ id: "a", expires_at: 10 }] }], model_routes: [], aliases: {} };
  base.routeConfig.secrets = [{ path: "/providers/0/credentials/0/api_key", configured: true, fingerprint: "old" }];
  const latest = structuredClone(base);
  latest.routeConfig.revision.id = "r2";
  (latest.routeConfig.document.providers[0] as { credentials: { expires_at: number }[] }).credentials[0].expires_at = 20;
  latest.routeConfig.secrets[0].fingerprint = "rotated";
  const draft: ConsoleRouteConfigCommitRequest = { expectedRevision: base.routeConfig.revision.id,
    document: structuredClone(base.routeConfig.document), secretPatches: [{ path: "/providers/0/credentials/0/api_key", operation: "keep" }] };
  (draft.document.providers[0] as Record<string, unknown>).label = "After";
  vi.mocked(api.getRouteConfig).mockClear().mockResolvedValue(latest);
  vi.mocked(api.commitRouteConfig).mockRejectedValueOnce(conflict());
  const action = { api, generation: 1, managementToken: "test", secretGrant: "grant", secretGrantEpoch: 1 };
  return { api, base, latest, draft, action };
}

it("recovers the real token-refresh conflict with one fresh CAS commit", async () => {
  const f = await fixture();
  await commitRouteDraft(f.action, f.base, f.draft, () => true);
  expect(f.api.commitRouteConfig).toHaveBeenCalledTimes(2);
  const second = vi.mocked(f.api.commitRouteConfig).mock.calls[1];
  expect(second[1]).toMatchObject({ expectedRevision: "r2", document: { providers: [{ label: "After", credentials: [{ expires_at: 20 }] }] },
    secretPatches: [{ path: "/providers/0/credentials/0/api_key", operation: "keep" }] });
  expect(second[2]).toBe("grant");
  expect(f.api.getRouteConfig).toHaveBeenCalledTimes(1);
});

it("does not repeat a conflicting field or overwrite a rotated secret", async () => {
  const f = await fixture();
  (f.latest.routeConfig.document.providers[0] as Record<string, unknown>).label = "Remote";
  await expect(commitRouteDraft(f.action, f.base, f.draft, () => true)).rejects.toThrow(/冲突/);
  expect(f.api.commitRouteConfig).toHaveBeenCalledTimes(1);
  const g = await fixture();
  g.draft.secretPatches[0] = { path: g.draft.secretPatches[0].path, operation: "replace", value: "new-key" };
  await expect(commitRouteDraft(g.action, g.base, g.draft, () => true)).rejects.toThrow(/冲突/);
  expect(g.api.commitRouteConfig).toHaveBeenCalledTimes(1);
});

it("rebases unrelated edits without changing the target of a staged key", async () => {
  const f = await fixture();
  f.latest.routeConfig.document = structuredClone(f.base.routeConfig.document);
  f.latest.routeConfig.document.aliases = { remote: "model" };
  f.latest.routeConfig.secrets = structuredClone(f.base.routeConfig.secrets);
  f.draft.secretPatches[0] = { path: f.draft.secretPatches[0].path, operation: "replace", value: "new-key" };
  await commitRouteDraft(f.action, f.base, f.draft, () => true);
  expect(vi.mocked(f.api.commitRouteConfig).mock.calls[1][1]).toMatchObject({ expectedRevision: "r2",
    document: { aliases: { remote: "model" } }, secretPatches: [{ operation: "replace", value: "new-key" }] });
});

it("stops after one retry and never retries a network error", async () => {
  const f = await fixture();
  vi.mocked(f.api.commitRouteConfig).mockRejectedValueOnce(conflict());
  await expect(commitRouteDraft(f.action, f.base, f.draft, () => true)).rejects.toThrow(/仍在更新/);
  expect(f.api.commitRouteConfig).toHaveBeenCalledTimes(2);
  const g = await fixture();
  vi.mocked(g.api.commitRouteConfig).mockReset().mockRejectedValue(new Error("offline"));
  await expect(commitRouteDraft(g.action, g.base, g.draft, () => true)).rejects.toThrow("offline");
  expect(g.api.getRouteConfig).not.toHaveBeenCalled();
});

it("does not replay against a changed login or gateway while fetching the latest revision", async () => {
  const f = await fixture(); const pending = deferred<typeof f.latest>(); let current = true;
  vi.mocked(f.api.getRouteConfig).mockReturnValue(pending.promise);
  const result = commitRouteDraft(f.action, f.base, f.draft, () => current);
  await Promise.resolve(); current = false; pending.resolve(f.latest);
  expect(await result).toBeNull();
  expect(f.api.commitRouteConfig).toHaveBeenCalledTimes(1);
});
