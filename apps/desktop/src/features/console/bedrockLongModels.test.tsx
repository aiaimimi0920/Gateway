import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { applyModelSelection } from "./credentialModelSelection";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { testProviderContext } from "./credentialTestPolicyDocument";
import { namedPlanError } from "./credentialTestPlansDocument";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { CredentialDialog } from "./CredentialDialog";
import { ProviderModelMappingDialog } from "./ProviderModelMappingDialog";
import { modelIdPolicy, validModelId } from "./providerModelId";
import { testScopeModelPolicy } from "./credentialTestPolicyDocument";

const prefix = "arn:aws:bedrock:us-east-1:123456789012:application-inference-profile/";
const arn = prefix + "A".repeat(2048 - prefix.length);
const doc = () => ({ providers: [{ id: "bedrock", preset: "bedrock-converse", credentials: [{ id: "one" }] }], model_routes: [], aliases: {} });

it("keeps a full Bedrock ARN and its fetched/manual ownership across save and reopen", () => {
  const saved = applyModelSelection(doc(), "bedrock", "one", { manual: [arn], fetched: ["short", arn] });
  const value = credentialDialogValueFromDocument(JSON.parse(JSON.stringify(saved)), "bedrock", "one")!;
  expect(value.manualModelsText).toBe(arn);
  expect(value.fetchedModelsText).toBe(`short\n${arn}`);
  expect(testProviderContext(saved, "bedrock")!.models).toContain(arn);
  expect(saved.model_routes).toContainEqual({ pattern: arn, provider_ids: ["bedrock"], priority: 10, enabled: true });
});

it("does not trust a provider ID suffix or relax normal model byte bounds", () => {
  const ordinary = doc(); ordinary.providers[0].preset = "openai";
  for (const id of [arn, "a".repeat(257), "模".repeat(86)]) {
    expect(() => applyModelSelection(ordinary, "bedrock", "one", { manual: [id], fetched: [] })).toThrow();
  }
  for (const id of [arn + "A", arn + "/../bad", arn + "?"]) {
    expect(() => applyModelSelection(doc(), "bedrock", "one", { manual: [id], fetched: [] })).toThrow();
  }
});

it("saves a named Bedrock plan without enlarging its stable plan or case IDs", () => {
  const plan = {
    id: "named", name: "Named", scopes: [{ kind: "account" as const, id: "one" }],
    policy: {
      automaticEnabled: true, intervalMinutes: 60, modelSelection: "selected" as const, models: [arn],
      cases: [{ id: "case", name: "Case", prompt: "Reply OK", difficulty: 1, enabled: true }]
    }
  };
  expect(namedPlanError(doc(), { providerId: "bedrock", id: "named", plan })).toBeNull();
  const ordinary = doc(); ordinary.providers[0].preset = "openai";
  expect(namedPlanError(ordinary, { providerId: "bedrock", id: "named", plan })).not.toBeNull();
  plan.policy.cases[0].id = "a".repeat(257);
  expect(namedPlanError(doc(), { providerId: "bedrock", id: "named", plan })).not.toBeNull();
});

it("uses actual transport context rather than a preset hidden by another adapter or discovery", () => {
  expect(modelIdPolicy({ preset: "bedrock-converse", adapter: "openai_compatible" })).toBe("standard");
  expect(modelIdPolicy({ preset: "bedrock-converse", call_flow: { call_adapter: "openai" } })).toBe("standard");
  expect(modelIdPolicy({ preset: "bedrock-converse", callFlow: { call_adapter: "openai" } })).toBe("standard");
  expect(modelIdPolicy({ preset: "bedrock-converse" }, { discovery: { protocol: "chat_completions" } })).toBe("standard");
  expect(validModelId("模".repeat(86), "bedrock")).toBe(false);
  const mixed = doc(); Object.assign(mixed.providers[0], { credentials: [{ id: "one" }, { id: "two", discovery: { protocol: "chat_completions" } }] });
  expect(testScopeModelPolicy(mixed, [{ providerId: "bedrock", kind: "pool" }])).toBe("standard");
  expect(testScopeModelPolicy(mixed, [{ providerId: "bedrock", kind: "account", id: "one" }])).toBe("bedrock");
  Object.assign(mixed.providers[0], { supported_models: [arn] });
  expect(testProviderContext(mixed, "bedrock")!.accounts[1].models).toEqual([]);
});

it("applies metadata-only edits through the normal controller without manufacturing a discovery job", () => {
  const saved = applyModelSelection(doc(), "bedrock", "one", { manual: [arn], fetched: [] });
  const initial = credentialDialogValueFromDocument(saved, "bedrock", "one")!;
  const replaceEditorDocument = vi.fn(); const setError = vi.fn();
  const { result } = renderHook(() => useCredentialDialogEditor({
    editorText: JSON.stringify(saved),
    credentialDialogState: { mode: "edit", initialValue: initial }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits: vi.fn(), setError, replaceEditorDocument, t: (zh) => zh
  }));
  act(() => result.current.applyCredentialDialogValue(initial));
  expect(setError).toHaveBeenLastCalledWith(null);
  expect(replaceEditorDocument.mock.lastCall![0].providers[0].credentials[0].model_selection.manual).toEqual([arn]);
  expect(JSON.stringify(replaceEditorDocument.mock.lastCall![0])).not.toContain("discovery_job");
});

it("submits a full ARN from the credential dialog without requiring secret replacement", () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const saved = applyModelSelection(doc(), "bedrock", "one", { manual: [arn], fetched: [] });
  const onSubmit = vi.fn();
  render(<NeuroProvider><UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "bedrock", label: "Bedrock" }]}
    existingCredentialIds={["one"]} initialValue={credentialDialogValueFromDocument(saved, "bedrock", "one")!}
    bedrockProvider={() => saved.providers[0] as Record<string, unknown>} locked={false} hasSecretAccess={false}
    onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider></NeuroProvider>);
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ supportedModelsText: arn, apiKeyOperation: "keep" }));
});

it("autosaves long upstream targets from the mapping dialog unchanged", () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const onSubmit = vi.fn();
  render(<NeuroProvider><UiLocaleProvider><ProviderModelMappingDialog open providerId="renamed" providerLabel="Bedrock" locked={false}
    modelPolicy="bedrock" modelOptions={["alias"]} upstreamModelOptions={[]} initialEntries={[]}
    onOpenChange={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider></NeuroProvider>);
  fireEvent.change(screen.getByLabelText("自定义映射模型"), { target: { value: arn } });
  fireEvent.click(screen.getByRole("button", { name: "添加映射模型" }));
  expect(onSubmit).toHaveBeenCalledWith([{ model: "alias", upstreamModel: arn }]);
});
