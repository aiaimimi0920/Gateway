import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog } from "./CredentialDialog";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { useCredentialDialogEditor } from "./useCredentialDialogEditor";
import { CardModelList } from "./CardModelList";
import type { NvidiaCatalog } from "./nvidiaModelCatalog";
import { buildAccountLedgerSections } from "./accountLedgerSections";
import { buildAccountLedgerRows, buildRouteAccountCatalog } from "./accountManagementViewModel";
import { buildConsoleTelemetrySnapshot } from "./telemetry";
import { editModelSources } from "./credentialModelSelection";

const catalog: NvidiaCatalog = { sourceUrl: "https://integrate.api.nvidia.com/v1/models",
  checkedAt: "2026-10-26T04:00:00Z", models: ["openai/new"] };
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("reads NVIDIA catalog before adding an account, without generic discovery", async () => {
  const onSubmit = vi.fn(); const onDiscover = vi.fn();
  const onDiscoverProviderModels = vi.fn().mockResolvedValue(catalog);
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "nv", label: "NVIDIA" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => "nvidia"} onDiscoverProviderModels={onDiscoverProviderModels} onDiscover={onDiscover} />
  </UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ modelCatalog: catalog })));
  expect(onDiscoverProviderModels).toHaveBeenCalledWith({ providerId: "nv", apiKey: "fixture-key" }, expect.any(AbortSignal));
  expect(onDiscover).not.toHaveBeenCalled();
});

it("does not save a new NVIDIA account when its catalog read fails", async () => {
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "nv", label: "NVIDIA" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} catalogProviderKind={() => "nvidia"} onDiscoverProviderModels={vi.fn().mockRejectedValue(new Error("offline"))} />
  </UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "new-account" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("offline"));
  expect(onSubmit).not.toHaveBeenCalled();
});

it("persists catalog metadata and synchronizes the account, pool and routes through ordinary editing", () => {
  const document = { providers: [{ id: "nv", preset: "nvidia-openai", supported_models: ["old", "manual"],
    credentials: [{ id: "one", supported_models: ["old", "manual"], model_catalog: { ...catalog, models: ["old"] } }] }],
    model_routes: [{ pattern: "old", provider_ids: ["nv"] }], aliases: {} };
  const initial = credentialDialogValueFromDocument(document, "nv", "one")!;
  expect(initial.modelCatalog?.models).toEqual(["old"]);
  const replaceEditorDocument = vi.fn(); const setCredentialSecretEdits = vi.fn();
  const hook = renderHook(() => useCredentialDialogEditor({ editorText: JSON.stringify(document),
    credentialDialogState: { mode: "edit", initialValue: initial }, setCredentialDialogState: vi.fn(),
    setCredentialSecretEdits, setError: vi.fn(), replaceEditorDocument, t: (zh) => zh }));
  act(() => hook.result.current.applyCredentialDialogValue(editModelSources({ ...initial, modelCatalog: catalog }, catalog.models.join("\n"), "manual")));
  const saved = replaceEditorDocument.mock.calls[0][0];
  expect(saved.providers[0]).toMatchObject({ supported_models: ["manual", "openai/new"],
    credentials: [{ id: "one", supported_models: ["manual", "openai/new"], model_catalog: catalog }] });
  expect(saved.model_routes).toEqual([{ pattern: "openai/new", provider_ids: ["nv"], priority: 10, enabled: true },
    { pattern: "manual", provider_ids: ["nv"], priority: 10, enabled: true }]);
  expect(JSON.stringify(saved)).not.toContain("verified_models");
  expect(setCredentialSecretEdits.mock.calls[0][0]([])).toEqual([]);
});

it("keeps unverified evidence while showing successful calls through metrics without an extra caption", () => {
  const { container } = render(<CardModelList models={["openai/discovered", "openai/success", "openai/failed", "openai/manual"]}
    label="NVIDIA" t={(zh) => zh} traffic={{ emptyRequestCount: 0,
      catalogModels: ["openai/discovered", "openai/success", "openai/failed"], models: new Map([
        ["openai/success", { requestCount: 10, successCount: 9, successWindows: [] }],
        ["openai/failed", { requestCount: 1, successCount: 0, successWindows: [] }],
      ]) }} />);
  for (const [model, state] of [["discovered", "目录已发现"], ["failed", "尚无成功"], ["manual", "未验证"]]) {
    expect(container.querySelector(`[data-card-model="openai/${model}"]`)).toHaveTextContent(state);
  }
  expect(container.querySelector('[data-card-model="openai/success"] .nt-card-models__evidence')).toBeNull();
  expect(screen.queryByText("有成功记录")).not.toBeInTheDocument();
  expect(screen.queryByText("可调用模型")).not.toBeInTheDocument();
  expect(screen.getByText(/模型池/)).toBeInTheDocument();
});

it("carries account catalog evidence to the account and pool cards without changing availability", () => {
  const document = { providers: [{ id: "nv", preset: "nvidia-openai", base_url: "https://integrate.api.nvidia.com",
    credentials: [{ id: "one", supported_models: catalog.models, model_catalog: catalog }] }], model_routes: [], aliases: {} };
  const rows = buildAccountLedgerRows(buildRouteAccountCatalog(document), {});
  const snapshot = buildConsoleTelemetrySnapshot({ pressure: null, costOverview: null,
    credentialModelStates: [], credentialInventory: null, requestAuditSummary: null });
  const sections = buildAccountLedgerSections(document, rows, new Map(), snapshot, "zh-CN");
  expect(sections[0].modelTraffic?.catalogModels).toEqual(catalog.models);
  expect(sections[0].directAccounts[0]).toMatchObject({ discoverySupported: true, statusLabel: "待观测" });
  expect(sections[0].directAccounts[0].modelTraffic?.catalogModels).toEqual(catalog.models);
  expect(sections[0].directAccounts[0].modelTraffic?.models.size).toBe(0);
});
