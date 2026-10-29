import { renderHook } from "@testing-library/react";
import { expect, it } from "vitest";
import { parseRouteDocument } from "./routeDocument";
import { useConsoleCredentialSelectors } from "./useConsoleCredentialSelectors";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";

it("uses access-qualified account options without rewriting source labels or IDs", () => {
  // Reproduce legacy UTF-8 bytes incorrectly decoded as Latin-1 twice.
  const misdecode = (value: string) => Array.from(new TextEncoder().encode(value),
    (byte) => String.fromCharCode(byte)).join("");
  const corruptedLabel = misdecode(misdecode("Muyuan · 第三方 OpenAI 兼容"));
  const document = parseRouteDocument(JSON.stringify({
    providers: [
      { id: "muyuan-openai", label: corruptedLabel },
      { id: "openrouter", preset: "openai", label: "Old name" },
      { id: "custom-relay", label: "我的自定义站点" },
    ], model_routes: [], aliases: {},
  }));
  const before = JSON.stringify(document);
  const { result } = renderHook(() => useConsoleCredentialSelectors({
    credentialPoolAutomation: null,
    credentialRefill: null,
    draftDocumentState: { document, invalid: false },
    draftAccountCatalog: { groups: [], accounts: [], providerBuckets: [], ungroupedCount: 0 },
  }));
  expect(result.current.credentialProviderOptions).toEqual([
    { id: "muyuan-openai", label: "Muyuan · 第三方 OpenAI 兼容-api" },
    { id: "openrouter", label: "Old name-api" },
    { id: "custom-relay", label: "我的自定义站点-api" },
  ]);
  expect(JSON.stringify(document)).toBe(before);
});

it("keeps every catalog template's canonical name in account provider options", () => {
  const providers = PROVIDER_CATALOG_TEMPLATES.map((template) => ({
    id: template.providerId, label: template.labelZh, preset: template.preset,
    adapter: template.adapter, protocol_profile: template.protocolProfile,
  }));
  const document = parseRouteDocument(JSON.stringify({ providers, model_routes: [], aliases: {} }));
  const before = JSON.stringify(document);
  const { result } = renderHook(() => useConsoleCredentialSelectors({
    credentialPoolAutomation: null, credentialRefill: null,
    draftDocumentState: { document, invalid: false },
    draftAccountCatalog: { groups: [], accounts: [], providerBuckets: [], ungroupedCount: 0 },
  }));
  expect(result.current.credentialProviderOptions).toEqual(
    PROVIDER_CATALOG_TEMPLATES.map((template) => ({ id: template.providerId, label: template.providerId })),
  );
  expect(JSON.stringify(document)).toBe(before);
});

it("distinguishes legacy ChatGPT modes using protocol metadata, not the old display name", () => {
  const document = parseRouteDocument(JSON.stringify({ providers: [
    { id: "saved-codex", label: "ChatGPT", preset: "chatgpt-codex-oauth-official-api" },
    { id: "saved-web", label: "ChatGPT", adapter: "chatgpt_web_reverse_compatible" },
    { id: "saved-api", label: "openai-web", protocol_profile: "openai" },
  ], model_routes: [], aliases: {} }));
  const { result } = renderHook(() => useConsoleCredentialSelectors({
    credentialPoolAutomation: null, credentialRefill: null,
    draftDocumentState: { document, invalid: false },
    draftAccountCatalog: { groups: [], accounts: [], providerBuckets: [], ungroupedCount: 0 },
  }));
  expect(result.current.credentialProviderOptions).toEqual([
    { id: "saved-codex", label: "codex-oauth" },
    { id: "saved-web", label: "ChatGPT-web" },
    { id: "saved-api", label: "openai-api" },
  ]);
});
