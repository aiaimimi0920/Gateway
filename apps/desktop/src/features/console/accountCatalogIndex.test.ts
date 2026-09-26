import { describe, expect, it } from "vitest";

import { indexAccountsByProvider } from "./accountCatalogIndex";

describe("indexAccountsByProvider", () => {
  it("groups accounts in one pass and preserves their display order", () => {
    let providerReads = 0;
    const account = (id: string, providerId: string) => ({
      id,
      get providerId() {
        providerReads += 1;
        return providerId;
      },
    });
    const accounts = [
      account("alpha", "provider-a"),
      account("beta", "provider-b"),
      account("gamma", "provider-a"),
    ];

    const result = indexAccountsByProvider(accounts);

    expect(result.get("provider-a")?.map(({ id }) => id)).toEqual(["alpha", "gamma"]);
    expect(result.get("provider-b")?.map(({ id }) => id)).toEqual(["beta"]);
    expect(providerReads).toBe(accounts.length);
  });

  it("returns an empty index for an empty account catalog", () => {
    expect(indexAccountsByProvider([]).size).toBe(0);
  });
});
