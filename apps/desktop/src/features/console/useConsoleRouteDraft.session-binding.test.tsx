import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteConfigResponse } from "../../api/contracts";
import { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import { parseProviderSessionMaterial } from "./providerSessionMaterial";
import { applyProviderSessionMaterial } from "./providerSessionMaterialApply";

it("keeps invalid pending sessions out of render previews and rejects them at the real commit owner", () => {
  const document = { providers: [{ id: "web", preset: "grok", base_url: "https://grok.com", explicit_credentials_only: true, credentials: [{ id: "a" }] }], model_routes: [], aliases: {} };
  const routeConfig: ConsoleRouteConfigResponse = { routeConfig: { revision: { id: "fixture-r1", sequence: 1 }, source: "local", document,
    diagnostics: { diagnostics: [] }, requiresRepair: false, mutationSupported: true, secrets: [] } };
  const options = { routeConfig, hasSecretAccess: true, t: (zh: string) => zh, invalidateCredentialProbes: vi.fn(),
    accountGroupDraftRowsFromDocument: () => [], onRouteConfigHydrated: vi.fn(), setError: vi.fn(), setSecretDialogOpen: vi.fn() };
  const { result } = renderHook(() => useConsoleRouteDraft(options));
  const imported = applyProviderSessionMaterial(document, "web", "a", parseProviderSessionMaterial(JSON.stringify({ product: "grok-web", token: "synthetic-staged-session" }), "grok-web"));
  act(() => { result.current.replaceEditorDocument(imported.document); result.current.setCredentialSecretEdits(imported.secretEdits); });
  expect(result.current.parseDraft().secretPatches).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-session" });
  act(() => { result.current.setEditorText(result.current.editorText.replace("https://grok.com", "https://evil.test")); });
  expect(JSON.stringify(result.current.secretPatches)).not.toContain("synthetic-staged-session");
  expect(() => result.current.parseDraft()).toThrow("目标地址");
  act(() => { result.current.setEditorText(result.current.editorText.replace("https://evil.test", "https://grok.com")); });
  expect(result.current.parseDraft().secretPatches).toContainEqual({ path: "/providers/0/credentials/0/api_key", operation: "replace", value: "synthetic-staged-session" });
});
