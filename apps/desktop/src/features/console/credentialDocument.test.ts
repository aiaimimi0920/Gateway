import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { addExplicitCredential, addProviderWithCredential } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

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

  it("adds a provider with its first account and joins existing or new model routes", () => {
    const source: ConsoleRouteDocument = {
      providers: [
        {
          id: "primary-provider",
          credentials: [{ id: "primary-account" }],
        },
      ],
      model_routes: [
        {
          pattern: "shared-model",
          provider_ids: ["primary-provider"],
          priority: 10,
        },
      ],
      aliases: {},
      account_groups: [],
    };

    const result = addProviderWithCredential(source, {
      provider: {
        id: "third-party-openai",
        label: "Third-party OpenAI-compatible",
        adapter: "openai_compatible",
        protocol_profile: "openai_compatible_generic",
        base_url: "https://gateway.example.test/v1",
        supported_models: ["shared-model", "exclusive-model"],
      },
      credential: {
        id: "third-party-account-1",
        account_name: "Third-party Account 1",
        enabled: true,
      },
      routePatterns: ["shared-model", "exclusive-model", "shared-model"],
    });

    expect(result.providers).toHaveLength(2);
    expect(result.providers[1]).toMatchObject({
      id: "third-party-openai",
      adapter: "openai_compatible",
      protocol_profile: "openai_compatible_generic",
      credentials: [
        {
          id: "third-party-account-1",
          account_name: "Third-party Account 1",
          enabled: true,
        },
      ],
    });
    expect(result.model_routes).toEqual([
      {
        pattern: "shared-model",
        provider_ids: ["primary-provider", "third-party-openai"],
        priority: 10,
      },
      {
        pattern: "exclusive-model",
        provider_ids: ["third-party-openai"],
      },
    ]);
    expect(source.providers).toHaveLength(1);
    expect(source.model_routes[0]).toMatchObject({
      provider_ids: ["primary-provider"],
    });
  });

  it("rejects duplicate provider and credential identities when creating a provider", () => {
    const source = routeDocument([
      {
        id: "existing-provider",
        credentials: [{ id: "existing-account" }],
      },
    ]);

    expect(() =>
      addProviderWithCredential(source, {
        provider: { id: "existing-provider", base_url: "https://example.test" },
        credential: { id: "new-account" },
      }),
    ).toThrow(/provider 'existing-provider' already exists/i);
    expect(() =>
      addProviderWithCredential(source, {
        provider: { id: "new-provider", base_url: "https://example.test" },
        credential: { id: "existing-account" },
      }),
    ).toThrow(/credential 'existing-account' already exists/i);
  });

});
