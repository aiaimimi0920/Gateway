import { describe, expect, it } from "vitest";
import { deleteExplicitCredential } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

describe("credential document mutations", () => {
  it("deletes one explicit credential and removes only that identity from account groups", () => {
    const source = routeDocument(
      [
        {
          id: "openai-main",
          credentials: [
            { id: "openai-account-a", custom: "a" },
            { id: "openai-account-b", custom: "b" },
          ],
        },
      ],
      [
        {
          id: "premium",
          provider_credential_ids: [
            "openai-account-a",
            "openai-account-b",
            "shared-account",
          ],
        },
      ],
    );

    const result = deleteExplicitCredential(source, {
      providerId: "openai-main",
      credentialId: "openai-account-a",
    });

    expect(result.providers).toEqual([
      {
        id: "openai-main",
        credentials: [{ id: "openai-account-b", custom: "b" }],
      },
    ]);
    expect(result.account_groups).toEqual([
      {
        id: "premium",
        provider_credential_ids: ["openai-account-b", "shared-account"],
      },
    ]);
    expect(source.providers).toEqual([
      {
        id: "openai-main",
        credentials: [
          { id: "openai-account-a", custom: "a" },
          { id: "openai-account-b", custom: "b" },
        ],
      },
    ]);
  });

  it("migrates group membership back to the provider default when deleting the last credential", () => {
    const source = routeDocument(
      [
        {
          id: "openai-main",
          credentials: [{ id: "openai-account-a" }],
        },
      ],
      [
        {
          id: "premium",
          provider_credential_ids: ["openai-account-a"],
        },
      ],
    );

    const result = deleteExplicitCredential(source, {
      providerId: "openai-main",
      credentialId: "openai-account-a",
    });

    expect(result.providers).toEqual([{ id: "openai-main", credentials: [] }]);
    expect(result.account_groups).toEqual([
      {
        id: "premium",
        provider_credential_ids: ["openai-main::default"],
      },
    ]);
  });
});
