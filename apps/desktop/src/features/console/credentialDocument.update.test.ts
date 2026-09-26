import { describe, expect, it } from "vitest";
import { updateExplicitCredential, updateCredentialProbeSchedule, updateProviderProbeSchedule } from "./credentialDocument";
import { routeDocument } from "./credentialDocumentTestFixtures";

describe("credential document mutations", () => {
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

  it("updates scheduled probes for explicit and provider-default credentials without mutating the source", () => {
    const source = routeDocument([
      {
        id: "pooled",
        credentials: [{ id: "account-a", custom: "preserved" }],
      },
      {
        id: "single",
        base_url: "https://single.example.test/v1",
      },
    ]);

    const explicitResult = updateCredentialProbeSchedule(source, {
      providerId: "pooled",
      credentialId: "account-a",
      mode: "credential",
      enabled: true,
      intervalMinutes: 15,
    });
    const defaultResult = updateCredentialProbeSchedule(explicitResult, {
      providerId: "single",
      credentialId: "single::default",
      mode: "provider-default",
      enabled: false,
      intervalMinutes: 120,
    });

    expect(defaultResult.providers).toEqual([
      {
        id: "pooled",
        credentials: [
          {
            id: "account-a",
            custom: "preserved",
            scheduled_probe_enabled: true,
            scheduled_probe_interval_minutes: 15,
          },
        ],
      },
      {
        id: "single",
        base_url: "https://single.example.test/v1",
        scheduled_probe_enabled: false,
        scheduled_probe_interval_minutes: 120,
      },
    ]);
    expect(source.providers).toEqual([
      {
        id: "pooled",
        credentials: [{ id: "account-a", custom: "preserved" }],
      },
      {
        id: "single",
        base_url: "https://single.example.test/v1",
      },
    ]);
  });

  it("applies a provider schedule to every pooled credential without mutating the source", () => {
    const source = routeDocument([
      {
        id: "pooled",
        scheduled_probe_enabled: true,
        credentials: [
          { id: "account-a", custom: "a" },
          {
            id: "account-b",
            custom: "b",
            scheduled_probe_enabled: false,
            scheduled_probe_interval_minutes: 5,
          },
        ],
      },
    ]);

    const result = updateProviderProbeSchedule(source, {
      providerId: "pooled",
      enabled: true,
      intervalMinutes: 45,
    });

    expect(result.providers).toEqual([
      {
        id: "pooled",
        scheduled_probe_enabled: false,
        scheduled_probe_interval_minutes: 45,
        credentials: [
          {
            id: "account-a",
            custom: "a",
            scheduled_probe_enabled: true,
            scheduled_probe_interval_minutes: 45,
          },
          {
            id: "account-b",
            custom: "b",
            scheduled_probe_enabled: true,
            scheduled_probe_interval_minutes: 45,
          },
        ],
      },
    ]);
    expect(source.providers).toEqual([
      {
        id: "pooled",
        scheduled_probe_enabled: true,
        credentials: [
          { id: "account-a", custom: "a" },
          {
            id: "account-b",
            custom: "b",
            scheduled_probe_enabled: false,
            scheduled_probe_interval_minutes: 5,
          },
        ],
      },
    ]);
  });

  it("stores a provider schedule on the provider when it has no explicit credentials", () => {
    const source = routeDocument([
      {
        id: "default-provider",
        base_url: "https://default.example/v1",
      },
    ]);

    const result = updateProviderProbeSchedule(source, {
      providerId: "default-provider",
      enabled: true,
      intervalMinutes: 120,
    });

    expect(result.providers[0]).toEqual({
      id: "default-provider",
      base_url: "https://default.example/v1",
      scheduled_probe_enabled: true,
      scheduled_probe_interval_minutes: 120,
    });
    expect(source.providers[0]).not.toHaveProperty("scheduled_probe_enabled");
  });
});
