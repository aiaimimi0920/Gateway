import { expect, it } from "vitest";
import { parseRouteDocument } from "./routeDocument";
import {
  buildSiteHealthSecretPatches,
  healthSecretTarget,
  siteHealthSecretEditError,
  type SiteHealthSecretEdit,
} from "./siteHealthSecretDraft";

const document = () =>
  parseRouteDocument(
    JSON.stringify({
      providers: [
        {
          id: "one",
          base_url: "https://one.invalid/v1",
          site_health: {
            profiles: [
              {
                id: "plus",
                name: "Plus",
                policy: { adapter: "sub2api-v2", auth: { kind: "bearer" } },
              },
              { id: "pro", name: "Pro", policy: {} },
            ],
          },
        },
        { id: "two", base_url: "https://two.invalid", site_health: {} },
      ],
      model_routes: [],
      aliases: {},
    }),
  );
const edit = (): SiteHealthSecretEdit => ({
  providerId: "one",
  profileId: "plus",
  operation: "replace",
  value: "fixture-token",
  audience: healthSecretTarget(document(), "one", "plus")!.audience,
});

it("resolves pending tokens by provider and plan IDs after both arrays reorder", () => {
  const draft = document();
  draft.providers.reverse();
  (draft.providers[1] as any).site_health.profiles.reverse();
  expect(
    buildSiteHealthSecretPatches(
      draft,
      [{ path: "/other", operation: "keep" }],
      [edit()],
    ),
  ).toEqual([
    { path: "/other", operation: "keep" },
    {
      path: "/providers/1/site_health/profiles/1/policy/auth/token",
      operation: "replace",
      value: "fixture-token",
    },
  ]);
});

it("never reapplies an unsubmitted token to a changed destination or replacement identity", () => {
  const draft = document();
  (draft.providers[0] as any).base_url = "https://changed.invalid";
  expect(siteHealthSecretEditError(draft, [edit()])).toContain(
    "监控目标或认证已改变",
  );
  expect(buildSiteHealthSecretPatches(draft, [], [edit()])).toEqual([]);
  (draft.providers[0] as any).site_health.profiles[0].id = "replacement";
  expect(buildSiteHealthSecretPatches(draft, [], [edit()])).toEqual([]);
  expect(siteHealthSecretEditError(draft, [edit()])).toBeNull();
});

it("uses origin rather than business API path but fences adapter, auth and private network changes", () => {
  const changed = document();
  (changed.providers[0] as any).base_url = "https://one.invalid/v2";
  expect(siteHealthSecretEditError(changed, [edit()])).toBeNull();
  for (const patch of [
    { adapter: "channel-health" },
    { auth: { kind: "api_key", header: "x-api-key" } },
    { allow_private_network: true },
  ]) {
    const draft = document();
    Object.assign(
      (draft.providers[0] as any).site_health.profiles[0].policy,
      patch,
    );
    expect(siteHealthSecretEditError(draft, [edit()])).not.toBeNull();
  }
});
