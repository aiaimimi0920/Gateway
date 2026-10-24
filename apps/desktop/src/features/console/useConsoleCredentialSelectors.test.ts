import { renderHook } from "@testing-library/react";
import { expect, it } from "vitest";
import { parseRouteDocument } from "./routeDocument";
import { useConsoleCredentialSelectors } from "./useConsoleCredentialSelectors";

it("uses the shared brand for account options without rewriting source labels or IDs", () => {
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
    { id: "muyuan-openai", label: "Muyuan" },
    { id: "openrouter", label: "OpenRouter" },
    { id: "custom-relay", label: "我的自定义站点" },
  ]);
  expect(JSON.stringify(document)).toBe(before);
});
