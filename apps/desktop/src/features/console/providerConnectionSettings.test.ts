import { describe, expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { editProviderConnection, normalizeProviderTarget, providerTargetEditable } from "./providerConnectionDraft";

const original = "https://old.example/v1";
const target = "https://new.example/v1";
const fixture = (): ConsoleRouteDocument => ({ providers: [{
  id: "custom", label: "Before", adapter: "openai_compatible", base_url: original,
  protocol_profile: "openai_compatible_generic", protocol_family: "openai_chat",
  credentials: [
    { id: "inherits", discovery: { binding: "old" }, discovery_job: { status: "complete" }, model_catalog: { models: ["text"] },
      model_selection: { fetched: ["text"], manual: ["manual"] }, supported_models: ["manual", "text"], enabled: false },
    { id: "same", base_url: `${original}/`, discovery: { binding: "same-old" } },
    { id: "independent", base_url: "https://independent.example/v1", discovery: { binding: "own" } },
  ], supported_models: ["text", "manual"], model_map: { public: "text" },
}, { id: "other", adapter: "anthropic_compatible", base_url: "https://other.example" }],
model_routes: [{ pattern: "text", provider_ids: ["other", "custom"], enabled: false }],
account_groups: [{ id: "group", provider_credential_ids: ["inherits"] }], aliases: { test: "text" } });

describe("provider connection edits", () => {
  it("renames only public metadata, preserving identities, accounts and route priority", () => {
    const document = fixture();
    const next = editProviderConnection(document, "custom", JSON.stringify(document.providers[0]), { label: "New name", baseUrl: original });
    expect(next).toEqual({ ...document, providers: [{ ...document.providers[0] as object, label: "New name" }, document.providers[1]] });
    expect((document.providers[0] as Record<string, unknown>).label).toBe("Before");
  });
  it("repairs inherited and matching account targets, invalidating only their stale evidence", () => {
    const document = fixture();
    const next = editProviderConnection(document, "custom", JSON.stringify(document.providers[0]), { label: "After", baseUrl: ` ${target}/ ` });
    const provider = next.providers[0] as Record<string, unknown>;
    const accounts = provider.credentials as Record<string, unknown>[];
    expect(provider).toMatchObject({ id: "custom", label: "After", base_url: target, supported_models: ["text", "manual"], model_map: { public: "text" } });
    expect(accounts[0]).toEqual({ id: "inherits", model_selection: { fetched: ["text"], manual: ["manual"] }, supported_models: ["manual", "text"], enabled: false });
    expect(accounts[1]).toEqual({ id: "same", base_url: target });
    expect(accounts[2]).toEqual((document.providers[0] as { credentials: unknown[] }).credentials[2]);
    expect(next.model_routes).toEqual(document.model_routes);
    expect(next.account_groups).toEqual(document.account_groups);
    expect(next.providers[1]).toEqual(document.providers[1]);
  });
  it.each(["", "file:///tmp/target", "https://user:secret@example.com", "https://example.com?q=secret", "https://example.com/#x"])("rejects invalid target %s", value => {
    expect(() => normalizeProviderTarget(value)).toThrow();
  });
  it("rejects stale snapshots and retains specialized endpoint ownership", () => {
    const document = fixture();
    expect(() => editProviderConnection(document, "custom", "stale", { label: "After", baseUrl: target })).toThrow(/配置已变化/);
    const provider = document.providers[0] as Record<string, unknown>; provider.preset = "qwen-official-api";
    expect(providerTargetEditable(provider)).toBe(false);
    expect(() => editProviderConnection(document, "custom", JSON.stringify(provider), { label: "After", baseUrl: target })).toThrow(/专用目标/);
    expect(editProviderConnection(document, "custom", JSON.stringify(provider), { label: "After", baseUrl: original }).providers[0]).toMatchObject({ label: "After", base_url: original });
  });
});
