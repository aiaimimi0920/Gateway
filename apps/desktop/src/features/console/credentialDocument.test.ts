import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import {
  addExplicitCredential,
  buildCredentialSecretPatches,
  deleteExplicitCredential,
  updateExplicitCredential,
} from "./credentialDocument";

function routeDocument(
  providers: unknown[],
  accountGroups: unknown[] = [],
): ConsoleRouteDocument {
  return {
    providers,
    model_routes: [],
    aliases: {},
    account_groups: accountGroups,
  };
}

describe("credential document mutations", () => {
  it("adds the first explicit credential by provider identity and migrates default group membership", () => {
    const source = routeDocument(
      [
        {
          id: "openai-main",
          label: "OpenAI Main",
          vendor_extension: { lane: "primary" },
          credentials: [],
        },
      ],
      [
        {
          id: "premium",
          name: "Premium",
          provider_credential_ids: ["openai-main::default", "shared-account"],
        },
      ],
    );

    const result = addExplicitCredential(source, {
      providerId: "openai-main",
      credential: {
        id: "openai-account-a",
        account_name: "OpenAI Account A",
        custom_metadata: { owner: "gateway" },
      },
    });

    expect(result).not.toBe(source);
    expect(result.providers).toEqual([
      {
        id: "openai-main",
        label: "OpenAI Main",
        vendor_extension: { lane: "primary" },
        credentials: [
          {
            id: "openai-account-a",
            account_name: "OpenAI Account A",
            custom_metadata: { owner: "gateway" },
          },
        ],
      },
    ]);
    expect(result.account_groups).toEqual([
      {
        id: "premium",
        name: "Premium",
        provider_credential_ids: ["openai-account-a", "shared-account"],
      },
    ]);
    expect(source).toEqual(
      routeDocument(
        [
          {
            id: "openai-main",
            label: "OpenAI Main",
            vendor_extension: { lane: "primary" },
            credentials: [],
          },
        ],
        [
          {
            id: "premium",
            name: "Premium",
            provider_credential_ids: ["openai-main::default", "shared-account"],
          },
        ],
      ),
    );
  });

  it("rejects a credential ID that already exists under another provider", () => {
    const source = routeDocument([
      {
        id: "provider-a",
        credentials: [{ id: "shared-credential" }],
      },
      {
        id: "provider-b",
        credentials: [],
      },
    ]);

    expect(() =>
      addExplicitCredential(source, {
        providerId: "provider-b",
        credential: { id: "shared-credential" },
      }),
    ).toThrow(/credential 'shared-credential' already exists/i);
    expect(source.providers).toEqual([
      {
        id: "provider-a",
        credentials: [{ id: "shared-credential" }],
      },
      {
        id: "provider-b",
        credentials: [],
      },
    ]);
  });

  it("updates an explicit credential by stable identity without dropping unknown fields", () => {
    const source = routeDocument([
      {
        id: "openai-main",
        credentials: [
          {
            id: "openai-account-a",
            account_name: "Old name",
            enabled: true,
            base_url: "https://old.example.com/v1",
            supported_models: ["gpt-old"],
            custom_metadata: { owner: "gateway", tier: 2 },
            endpoint_execution_modes: { chat: "direct_http" },
          },
        ],
      },
    ]);

    const result = updateExplicitCredential(
      source,
      { providerId: "openai-main", credentialId: "openai-account-a" },
      {
        id: "attempted-identity-change",
        account_name: "New name",
        enabled: false,
        base_url: "https://new.example.com/v1",
        supported_models: ["gpt-5.4"],
      },
    );

    expect(result.providers).toEqual([
      {
        id: "openai-main",
        credentials: [
          {
            id: "openai-account-a",
            account_name: "New name",
            enabled: false,
            base_url: "https://new.example.com/v1",
            supported_models: ["gpt-5.4"],
            custom_metadata: { owner: "gateway", tier: 2 },
            endpoint_execution_modes: { chat: "direct_http" },
          },
        ],
      },
    ]);
    expect(source.providers).toEqual([
      {
        id: "openai-main",
        credentials: [
          {
            id: "openai-account-a",
            account_name: "Old name",
            enabled: true,
            base_url: "https://old.example.com/v1",
            supported_models: ["gpt-old"],
            custom_metadata: { owner: "gateway", tier: 2 },
            endpoint_execution_modes: { chat: "direct_http" },
          },
        ],
      },
    ]);
  });

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

  it("rebases keep patches by stable identities, drops deleted credentials, and adds a new API-key replacement", () => {
    const activeDocument = routeDocument([
      {
        id: "other-provider",
        api_key: "",
        credentials: [],
      },
      {
        id: "openai-main",
        api_key: "",
        credentials: [
          { id: "deleted-account", api_key: null },
          { id: "kept-account", api_key: null, auth_token: null },
        ],
      },
    ]);
    const draftDocument = routeDocument([
      {
        id: "openai-main",
        api_key: "",
        credentials: [
          { id: "new-account", api_key: null },
          { id: "kept-account", api_key: null, auth_token: null },
        ],
      },
      {
        id: "other-provider",
        api_key: "",
        credentials: [],
      },
    ]);

    const patches = buildCredentialSecretPatches({
      activeDocument,
      draftDocument,
      activeSecrets: [
        { path: "/providers/0/api_key", configured: true },
        { path: "/providers/1/api_key", configured: true },
        { path: "/providers/1/credentials/0/api_key", configured: true },
        { path: "/providers/1/credentials/1/api_key", configured: true },
        { path: "/providers/1/credentials/1/auth_token", configured: true },
      ],
      credentialSecretEdits: [
        {
          providerId: "openai-main",
          credentialId: "new-account",
          field: "api_key",
          operation: "replace",
          value: "sk-new-account",
        },
      ],
    });

    expect(patches).toEqual([
      { path: "/providers/0/api_key", operation: "keep" },
      {
        path: "/providers/0/credentials/0/api_key",
        operation: "replace",
        value: "sk-new-account",
      },
      { path: "/providers/0/credentials/1/api_key", operation: "keep" },
      { path: "/providers/0/credentials/1/auth_token", operation: "keep" },
      { path: "/providers/1/api_key", operation: "keep" },
    ]);
    expect(patches.some((patch) => patch.path.includes("deleted-account"))).toBe(false);
    expect(patches).not.toContainEqual({
      path: "/providers/0/credentials/0/api_key",
      operation: "keep",
    });
  });

  it("generates an API-key clear patch for a newly added credential", () => {
    const activeDocument = routeDocument([
      { id: "openai-main", credentials: [] },
    ]);
    const draftDocument = routeDocument([
      {
        id: "openai-main",
        credentials: [{ id: "new-account", api_key: null }],
      },
    ]);

    const patches = buildCredentialSecretPatches({
      activeDocument,
      draftDocument,
      activeSecrets: [],
      credentialSecretEdits: [
        {
          providerId: "openai-main",
          credentialId: "new-account",
          field: "api_key",
          operation: "clear",
        },
      ],
    });

    expect(patches).toEqual([
      {
        path: "/providers/0/credentials/0/api_key",
        operation: "clear",
      },
    ]);
  });

  it("preserves active secret operations while rebasing paths and lets credential edits override them", () => {
    const activeDocument = routeDocument([
      { id: "other-provider", credentials: [] },
      {
        id: "openai-main",
        auth_token: null,
        credentials: [
          { id: "account-a", api_key: null },
          { id: "account-b", api_key: null },
        ],
      },
    ]);
    const draftDocument = routeDocument([
      {
        id: "openai-main",
        auth_token: null,
        credentials: [
          { id: "account-b", api_key: null },
          { id: "account-a", api_key: null },
        ],
      },
      { id: "other-provider", credentials: [] },
    ]);

    const patches = buildCredentialSecretPatches({
      activeDocument,
      draftDocument,
      activeSecrets: [
        { path: "/providers/1/auth_token", configured: true },
        { path: "/providers/1/credentials/0/api_key", configured: true },
        { path: "/providers/1/credentials/1/api_key", configured: true },
        { path: "/global/secret", configured: true },
      ],
      activeSecretPatches: [
        { path: "/providers/1/auth_token", operation: "clear" },
        {
          path: "/providers/1/credentials/0/api_key",
          operation: "replace",
          value: "edited-account-a",
        },
        {
          path: "/providers/1/credentials/1/api_key",
          operation: "replace",
          value: "stale-account-b",
        },
        { path: "/global/secret", operation: "replace", value: "global-value" },
      ],
      credentialSecretEdits: [
        {
          providerId: "openai-main",
          credentialId: "account-b",
          field: "api_key",
          operation: "clear",
        },
      ],
    });

    expect(patches).toEqual([
      { path: "/global/secret", operation: "replace", value: "global-value" },
      { path: "/providers/0/auth_token", operation: "clear" },
      { path: "/providers/0/credentials/0/api_key", operation: "clear" },
      {
        path: "/providers/0/credentials/1/api_key",
        operation: "replace",
        value: "edited-account-a",
      },
    ]);
  });
});
