import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { usePilotPolicyEditor } from "./usePilotPolicyEditor";
import { readPilotIdentityCategories } from "./pilotPoolPolicy";
import { isRecord } from "./routeDocument";

it("preserves existing category policy fields when adding a new category", () => {
  const provider = { id: "custom", credential_identity_categories: [
    { id: "existing", label: "Existing", pool_target_size: 73,
      auto_refill_enabled: true, auto_prune_enabled: true },
  ] };
  const document: ConsoleRouteDocument = { providers: [provider], model_routes: [], aliases: {} };
  const replace = vi.fn<(document: ConsoleRouteDocument, sync?: boolean) => void>();
  const { result } = renderHook(() => usePilotPolicyEditor({
    editorText: JSON.stringify(document), setError: vi.fn(), replaceEditorDocument: replace,
    credentialPoolAutomation: null, credentialPoolAutomationByProvider: new Map(),
    credentialRefill: null, t: (_zh, en) => en,
  }));
  vi.spyOn(window, "prompt").mockReturnValue("New Class");
  act(() => result.current.handleAddIdentityCategory("custom"));
  const updated = replace.mock.calls[0][0].providers[0];
  if (!isRecord(updated)) throw new Error("Expected provider document");
  expect(updated.credential_identity_categories).toEqual([
    ...provider.credential_identity_categories,
    { id: "new-class", label: "New Class", pool_target_size: 30,
      auto_refill_enabled: false, auto_prune_enabled: false },
  ]);
  expect(readPilotIdentityCategories(updated, "custom", "Custom")[0]).toEqual({
    id: "existing", label: "Existing", poolTargetSize: 73,
    autoRefillEnabled: true, autoPruneEnabled: true,
  });
  expect(provider.credential_identity_categories).toHaveLength(1);
});

it("reports a missing provider without changing the storage-path draft", () => {
  const setError = vi.fn();
  const replace = vi.fn();
  const { result } = renderHook(() => usePilotPolicyEditor({
    editorText: JSON.stringify({ providers: [], model_routes: [], aliases: {} }),
    setError, replaceEditorDocument: replace, credentialPoolAutomation: null,
    credentialPoolAutomationByProvider: new Map(), credentialRefill: null,
    t: (_zh, en) => en,
  }));
  let saved: boolean | undefined;
  act(() => { saved = result.current.updateProviderStoragePath("missing", "credential_storage_path", "/srv/pool"); });
  expect(saved).toBe(false);
  expect(setError).toHaveBeenCalledWith("Provider missing could not be found.");
  expect(replace).not.toHaveBeenCalled();
});
