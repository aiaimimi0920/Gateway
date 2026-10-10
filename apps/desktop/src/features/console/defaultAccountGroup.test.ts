import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { buildRouteAccountCatalog } from "./routeAccountCatalog";

function fixture(): ConsoleRouteDocument {
  return { providers: [
    { id: "explicit", credentials: [{ id: "one" }, { id: "two", enabled: false }, {}] },
    { id: "inherited" },
  ], model_routes: [], aliases: {}, account_groups: [
    { id: "chosen", name: "chosen", enabled: false, provider_credential_ids: ["one"] },
  ] };
}

describe("default account group", () => {
  it("covers unassigned legacy, disabled and provider-default accounts without changing the document", () => {
    const document = fixture();
    const before = JSON.stringify(document);
    const catalog = buildRouteAccountCatalog(document);
    expect(catalog.groups.find((group) => group.id === "default")?.providerCredentialIds)
      .toEqual(["two", "explicit-cred-2", "inherited::default"]);
    expect(catalog.accounts.find((account) => account.id === "one")?.groupIds).toEqual(["chosen"]);
    expect(catalog.ungroupedCount).toBe(0);
    expect(JSON.stringify(document)).toBe(before);
  });

  it("recomputes fallback when membership changes while preserving configured group settings", () => {
    const document = fixture();
    document.account_groups = [
      { id: "chosen", name: "chosen", provider_credential_ids: ["one", "two"] },
      { id: "default", name: "default", enabled: false, billing_multiplier: 2,
        provider_credential_ids: ["one", "two", "deleted"] },
    ];
    const group = buildRouteAccountCatalog(document).groups.find((item) => item.id === "default")!;
    expect(group.providerCredentialIds).toEqual(["explicit-cred-2", "inherited::default"]);
    expect(group.enabled).toBe(false);
    expect(group.billingMultiplier).toBe(2);
    document.account_groups = [];
    expect(buildRouteAccountCatalog(document).groups[0].providerCredentialIds).toHaveLength(4);
  });
});
