import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { usePilotPolicyEditor } from "./usePilotPolicyEditor";
import { readPilotIdentityCategories } from "./pilotPoolPolicy";
import { isRecord } from "./routeDocument";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildAccountLedgerRows, buildRouteAccountCatalog } from "./accountManagementViewModel";
import { buildConsoleTelemetrySnapshot } from "./telemetry";

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

it("changes an existing provider's client profile without rewriting pool state or credentials", () => {
  const provider = { id: "relay", api_key: "fixture", credentials: [{ id: "one", refresh_token: "fixture-refresh" }],
    site_health: {}, codex_basispoints_enabled: false };
  const document: ConsoleRouteDocument = { providers: [provider], model_routes: [], aliases: {} };
  const replace = vi.fn();
  const { result, rerender } = renderHook(({ text }) => usePilotPolicyEditor({
    editorText: text, setError: vi.fn(), replaceEditorDocument: replace,
    credentialPoolAutomation: null, credentialPoolAutomationByProvider: new Map(), credentialRefill: null,
    t: (_zh, en) => en,
  }), { initialProps: { text: JSON.stringify(document) } });
  act(() => result.current.updatePilotProviderPolicy("relay", { requestClientProfile: "claude_cli" }));
  expect(replace).toHaveBeenCalledWith({ ...document, providers: [{ ...provider, request_client_profile: "claude_cli" }] }, true);
  rerender({ text: JSON.stringify(replace.mock.calls[0][0]) });
  act(() => result.current.updatePilotProviderPolicy("relay", { requestClientProfile: "auto" }));
  expect(replace.mock.lastCall?.[0].providers[0]).toEqual(provider);
});

it("round-trips a flow-only fingerprint through the real card editor and ledger", () => {
  const flow = { call_adapter: "anthropic", fingerprint_adapter: "workbuddy", special_adapter: "none" };
  const provider = { id: "relay", adapter: "openai_compatible", api_key: "fixture", call_flow: flow };
  const document: ConsoleRouteDocument = { providers: [provider], model_routes: [], aliases: {} };
  const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
    credentialModelStates: [], credentialInventory: null, requestAuditSummary: null });
  const read = (doc: ConsoleRouteDocument) => buildAccountLedgerSections(doc,
    buildAccountLedgerRows(buildRouteAccountCatalog(doc), {}), new Map(), snapshot, "zh-CN")[0].requestClientProfile;
  expect(read(document)).toBe("workbuddy");
  const replace = vi.fn();
  const { result, rerender } = renderHook(({ text }) => usePilotPolicyEditor({ editorText: text,
    setError: vi.fn(), replaceEditorDocument: replace, credentialPoolAutomation: null,
    credentialPoolAutomationByProvider: new Map(), credentialRefill: null, t: (_zh, en) => en,
  }), { initialProps: { text: JSON.stringify(document) } });
  for (const profile of ["claude_cli", "auto"] as const) {
    act(() => result.current.updatePilotProviderPolicy("relay", { requestClientProfile: profile }));
    const updated = replace.mock.lastCall![0];
    expect(updated.providers[0].call_flow).toEqual({ ...flow, fingerprint_adapter: profile });
    expect(read(updated)).toBe(profile);
    rerender({ text: JSON.stringify(updated) });
  }
  expect(provider.call_flow).toEqual(flow);
});

it("keeps camelCase flow edits valid through the real card editor", () => {
  const provider = { id: "relay", adapter: "openai_compatible", callFlow: {
    call_adapter: "anthropic", fingerprint_adapter: "workbuddy", special_adapter: "none",
  } };
  const document: ConsoleRouteDocument = { providers: [provider], model_routes: [], aliases: {} };
  const replace = vi.fn();
  const { result } = renderHook(() => usePilotPolicyEditor({ editorText: JSON.stringify(document),
    setError: vi.fn(), replaceEditorDocument: replace, credentialPoolAutomation: null,
    credentialPoolAutomationByProvider: new Map(), credentialRefill: null, t: (_zh, en) => en,
  }));
  act(() => result.current.updatePilotProviderPolicy("relay", { requestClientProfile: "claude_cli" }));
  expect(replace.mock.lastCall?.[0].providers[0]).toEqual({ ...provider, request_client_profile: "claude_cli",
    callFlow: { ...provider.callFlow, fingerprint_adapter: "claude_cli" } });
});

it("rejects an API fingerprint edit for a session-owned flow before replacing the draft", () => {
  const provider = { id: "web", preset: "gemini-web-chat", call_flow: {
    call_adapter: "auto", fingerprint_adapter: "session", special_adapter: "gemini_web",
  } };
  const document: ConsoleRouteDocument = { providers: [provider], model_routes: [], aliases: {} };
  const replace = vi.fn();
  const setError = vi.fn();
  const { result } = renderHook(() => usePilotPolicyEditor({ editorText: JSON.stringify(document),
    setError, replaceEditorDocument: replace, credentialPoolAutomation: null,
    credentialPoolAutomationByProvider: new Map(), credentialRefill: null, t: (_zh, en) => en,
  }));
  act(() => result.current.updatePilotProviderPolicy("web", { requestClientProfile: "claude_cli" }));
  expect(replace).not.toHaveBeenCalled();
  expect(setError).toHaveBeenCalledWith(expect.any(String));
});
