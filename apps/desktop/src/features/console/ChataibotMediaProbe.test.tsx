import { NeuroProvider } from "@neuro/ui";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { PilotActionDialog } from "./PilotActionDialog";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";

export function chataibotProbeProps(): ComponentProps<typeof PilotActionDialog> {
  const account = pilotAccount({ accountId: "exact", providerId: "chataibot", displayName: "Synthetic ChatAIBot" });
  const section = pilotSection({ providerId: "chataibot", providerIds: ["chataibot"], providerPreset: "chataibot", directAccounts: [account] });
  return { t: zh => zh, pilotActionDialog: { kind: "probe", providerId: "chataibot", account }, accountPilotSections: [section],
    testPolicyDocument: { providers: [{ id: "chataibot", preset: "chataibot", base_url: "https://chataibot.pro",
      call_flow: { special_adapter: "chataibot_web" }, supported_models: ["google-nano-banana-2", "ideogram"],
      model_map: { "my-image": "google-nano-banana-2" }, credentials: [{ id: "exact", supported_models: ["google-nano-banana-2", "ideogram"] }] }], model_routes: [], aliases: {} },
    closePilotActionDialog: vi.fn(), credentialProbeBusy: null, activePilotManagedAccount: null, activePilotProbeResult: null,
    draftDirty: false, draftMatchesActiveRevision: true, startPilotProbe: vi.fn(), providerProbeBusy: false, providerProbeError: null, providerProbeResponse: null,
    startProviderProbe: vi.fn().mockResolvedValue(undefined), editorLocked: false, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
    pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn(), telemetryError: null,
    activePilotStatsView: null, applyPilotProbeSchedule: vi.fn() };
}
it("opens the normal ChatAIBot image form without generating, saving or scheduling", () => {
  const p = chataibotProbeProps(); render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  expect(screen.getByRole("combobox", { name: "操作" })).toBeInTheDocument();
  expect(screen.queryByLabelText("启用自动测试")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "保存策略" })).not.toBeInTheDocument();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("requires a new cost confirmation each time and retains the configured alias and exact account", async () => {
  const p = chataibotProbeProps(), user = userEvent.setup(); render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.selectOptions(screen.getByRole("combobox", { name: "模型" }), "my-image");
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成图片");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(p.startProviderProbe).not.toHaveBeenCalled();
  expect(screen.getByRole("alertdialog")).toHaveTextContent("可能消耗供应商额度");
  await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(p.startProviderProbe).toHaveBeenCalledExactlyOnceWith({ prompt: "合成图片", model: "my-image", credentialIds: ["exact"],
    media: { operation: "image", confirmCharge: true, parameters: {} } }, "chataibot");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  await user.click(screen.getByRole("button", { name: "取消" }));
  expect(p.startProviderProbe).toHaveBeenCalledTimes(1);
});
it("requires local images for editing, supports four-image merge, and clears hidden sources on operation changes", async () => {
  const p = chataibotProbeProps(), user = userEvent.setup(); render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成编辑");
  await user.selectOptions(screen.getByRole("combobox", { name: "操作" }), "image_edit");
  expect(screen.queryByRole("option", { name: "ideogram" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  const files = [1, 2, 3, 4].map(i => new File([`synthetic-${i}`], `${i}.png`, { type: "image/png" }));
  await user.upload(screen.getByLabelText("源图片"), files);
  await waitFor(() => expect(screen.getByRole("button", { name: "运行测试" })).toBeEnabled());
  expect(p.startProviderProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "运行测试" })); await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(p.startProviderProbe).toHaveBeenCalledWith(expect.objectContaining({ media: { operation: "image_edit", confirmCharge: true,
    parameters: { images: files.map((_, i) => `data:image/png;base64,${btoa(`synthetic-${i + 1}`)}`) } } }), "chataibot");
  await user.selectOptions(screen.getByRole("combobox", { name: "操作" }), "image");
  await user.selectOptions(screen.getByRole("combobox", { name: "操作" }), "image_edit");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
});
it("invalidates pending consent on target, mapping, draft and revision changes", async () => {
  const p = chataibotProbeProps(), user = userEvent.setup(); const view = render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  view.rerender(<PilotActionDialog {...p} draftMatchesActiveRevision={false} />);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  view.rerender(<PilotActionDialog {...p} />); await user.click(screen.getByRole("button", { name: "运行测试" }));
  const next = structuredClone(p.testPolicyDocument!); Object.assign(next.providers[0] as object, { model_map: { "my-image": "ideogram" } });
  view.rerender(<PilotActionDialog {...p} testPolicyDocument={next} />);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  Object.assign(next.providers[0] as object, { base_url: "https://foreign.example.test" });
  view.rerender(<PilotActionDialog {...p} testPolicyDocument={structuredClone(next)} />);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument(); expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it.each(["dirty", "revision", "disabled", "provider-disabled", "missing", "models", "legacy"])("blocks %s before any cost authorization", async scenario => {
  const p = chataibotProbeProps(), user = userEvent.setup();
  const provider = p.testPolicyDocument!.providers[0] as Record<string, unknown>;
  const credentials = provider.credentials as Record<string, unknown>[];
  if (scenario === "dirty") p.draftDirty = true;
  if (scenario === "revision") p.draftMatchesActiveRevision = false;
  if (scenario === "disabled") credentials[0].enabled = false;
  if (scenario === "provider-disabled") provider.enabled = false;
  if (scenario === "missing") provider.credentials = [];
  if (scenario === "models") { provider.supported_models = []; provider.model_map = {}; credentials[0].supported_models = []; }
  if (scenario === "legacy") delete provider.call_flow;
  render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled(); expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("rejects too many, oversized and unsupported local image files without sending", async () => {
  const p = chataibotProbeProps(), user = userEvent.setup({ applyAccept: false }); render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成编辑");
  await user.selectOptions(screen.getByRole("combobox", { name: "操作" }), "image_edit");
  for (const files of [Array.from({ length: 5 }, () => new File(["x"], "x.png", { type: "image/png" })),
    [new File([new Uint8Array(135001)], "large.png", { type: "image/png" })], [new File(["x"], "x.txt", { type: "text/plain" })]]) {
    await user.upload(screen.getByLabelText("源图片"), files);
    expect(screen.getByText(/请选择 1–4 张/)).toBeInTheDocument(); expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  }
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
it("cancels its local round on close and isolates late results", async () => {
  const p = chataibotProbeProps(), user = userEvent.setup(); let resolve!: (value: unknown) => void;
  p.startProviderProbe = vi.fn().mockImplementation(() => new Promise(done => { resolve = done; }));
  render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" })); await user.click(screen.getByRole("button", { name: "确认生成" }));
  await user.click(screen.getByRole("button", { name: "关闭" }));
  expect(p.closePilotActionDialog).toHaveBeenCalledOnce();
  await act(async () => resolve({ result: { results: [{ credentialId: "exact", status: "passed", message: "late-image-success" }] } }));
  expect(screen.queryByText("late-image-success")).not.toBeInTheDocument(); expect(p.startProviderProbe).toHaveBeenCalledOnce();
});
it("renders only the exact account result and never treats foreign success as its own", async () => {
  const p = chataibotProbeProps(), user = userEvent.setup();
  p.startProviderProbe = vi.fn().mockResolvedValue({ result: { results: [
    { credentialId: "other", status: "passed", message: "foreign-image-success" },
    { credentialId: "exact", status: "failed", message: "exact-image-failure" }] } });
  render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  await user.click(screen.getByRole("button", { name: "运行测试" })); await user.click(screen.getByRole("button", { name: "确认生成" }));
  expect(await screen.findByText("exact-image-failure")).toBeInTheDocument();
  expect(screen.queryByText("foreign-image-success")).not.toBeInTheDocument();
});
it.each(["callFlow", "credential-auto"])("uses the compiled provider flow for %s without treating a credential block as an override", async scenario => {
  const p = chataibotProbeProps(), user = userEvent.setup();
  const provider = p.testPolicyDocument!.providers[0] as Record<string, unknown>;
  if (scenario === "callFlow") { provider.callFlow = provider.call_flow; delete provider.call_flow; }
  else (provider.credentials as Record<string, unknown>[])[0].call_flow = { special_adapter: "auto" };
  render(<PilotActionDialog {...p} />, { wrapper: NeuroProvider });
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeEnabled();
  expect(p.startProviderProbe).not.toHaveBeenCalled();
});
