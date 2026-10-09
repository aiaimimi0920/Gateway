import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderModelMappingDialog } from "./ProviderModelMappingDialog";
import { ProviderTestDialog } from "./ProviderTestDialog";
import { pilotSection, pilotAccount } from "./AccountsLedgerWorkspace.fixtures";
import type { ConsoleRouteDocument } from "../../api/contracts";

const testDocument: ConsoleRouteDocument = { providers: [{ id: "managed-provider", supported_models: ["a", "b"], credentials: [
  { id: "one", account_name: "One" }, { id: "two", account_name: "Two" },
] }], model_routes: [], aliases: {}, account_groups: [] };

it("autosaves multiple targets and closes without an extra write", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  const close = vi.fn();
  render(<UiLocaleProvider><ProviderModelMappingDialog open providerId="p" providerLabel="Pool"
    locked={false} modelOptions={["a", "other"]} upstreamModelOptions={["b", "c"]}
    initialEntries={[]} onOpenChange={close} onSubmit={onSubmit} /></UiLocaleProvider>);
  expect(within(screen.getByRole("region", { name: "用户模型" })).queryByRole("button", { name: "b" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("checkbox", { name: "b" }));
  await user.click(screen.getByRole("checkbox", { name: "c" }));
  expect(onSubmit).toHaveBeenCalledWith([{ model: "a", upstreamModel: "b" }, { model: "a", upstreamModel: "c" }]);
  await user.keyboard("{Escape}");
  expect(close).toHaveBeenCalledWith(false);
  expect(onSubmit).toHaveBeenCalledTimes(2);
});

it("uses one configuration panel and sends custom prompt only after explicit run", async () => {
  const user = userEvent.setup();
  const onProbe = vi.fn().mockResolvedValue(undefined);
  const apply = vi.fn();
  const close = vi.fn();
  const t = (zh: string) => zh;
  const section = pilotSection({ directAccounts: [pilotAccount({ accountId: "one", supportedModels: ["b"] }), pilotAccount({ accountId: "two" })] });
  render(<ProviderTestDialog document={testDocument} initialTab="auto" onProbe={onProbe} onSave={apply}
    probe={{ t, section, closePilotActionDialog: close, providerProbeBusy: false, providerProbeError: null,
      providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true }}
    schedule={{ t, section, closePilotActionDialog: close, editorLocked: false,
      pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(), pilotScheduleIntervalMinutes: "60",
      setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: apply }} />);
  expect(screen.queryByRole("tab")).not.toBeInTheDocument();
  expect(screen.queryByText("测试账户")).not.toBeInTheDocument();
  const dialog = screen.getByRole("dialog", { name: `测试 · ${section.providerLabel}` });
  expect(dialog.querySelector(".nt-pilot-dialog__hero")).toBeNull();
  expect(screen.queryByText(/自动计划与手动提示词测试分别管理/)).not.toBeInTheDocument();
  expect(screen.queryByText("服务商级自动测试")).not.toBeInTheDocument();
  expect(dialog.querySelector(".nt-pilot-dialog__hero")).toBeNull();
  expect(screen.queryByText(/可能消耗额度|账户独立配置优先|等级仅代表/)).not.toBeInTheDocument();
  expect(onProbe).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "清空测试集" }));
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  await user.type(screen.getByRole("textbox", { name: "临时提示词" }), "Hello from manual test");
  await user.click(screen.getByRole("button", { name: "加入测试集" }));
  await user.click(screen.getByRole("button", { name: "清空选择" }));
  await user.type(screen.getByRole("textbox", { name: "添加测试模型" }), "a");
  await user.click(screen.getByRole("button", { name: "添加模型" }));
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(onProbe).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ scope: { kind: "pool" }, credentialIds: ["one", "two"],
    testPlan: expect.objectContaining({ models: ["a"], modelSelection: "selected", cases: expect.arrayContaining([
      expect.objectContaining({ prompt: "Hello from manual test", enabled: true }),
    ]) }) }), "managed-provider");
  await user.type(screen.getByRole("textbox", { name: "添加测试模型" }), "*");
  await user.click(screen.getByRole("button", { name: "添加模型" }));
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  expect(apply).not.toHaveBeenCalled();
  await user.keyboard("{Escape}");
  expect(close).toHaveBeenCalled();
});

it("keeps compact schedule controls, validation and explicit saving", async () => {
  const user = userEvent.setup();
  const apply = vi.fn();
  const close = vi.fn();
  const t = (zh: string) => zh;
  const section = pilotSection();
  function Schedule() {
    return <ProviderTestDialog document={testDocument} initialTab="auto" onProbe={vi.fn()} onSave={(scope, policy) => { apply({ scope, policy }); return true; }}
      probe={{ t, section, closePilotActionDialog: close, providerProbeBusy: false,
        providerProbeError: null, providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true }}
      schedule={{ t, section, closePilotActionDialog: close, editorLocked: false,
        pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
        pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(),
        applyProviderProbeSchedule: vi.fn() }} />;
  }
  render(<Schedule />);
  await user.click(screen.getByRole("checkbox", { name: "启用自动测试" }));
  const interval = screen.getByRole("spinbutton", { name: "执行间隔（分钟）" });
  const save = screen.getByRole("button", { name: "保存策略" });
  expect(apply).not.toHaveBeenCalled();
  await user.clear(interval);
  await user.type(interval, "0");
  expect(save).toBeDisabled();
  expect(interval).toHaveAttribute("aria-invalid", "true");
  expect(screen.getByRole("status")).toHaveTextContent("间隔须为 1–10080 分钟的整数。");
  await user.clear(interval);
  await user.type(interval, "15");
  expect(screen.getByRole("status")).not.toHaveTextContent("间隔须为");
  await user.click(save);
  expect(apply).toHaveBeenCalledExactlyOnceWith({ scope: { kind: "pool", providerId: "managed-provider" }, policy: expect.objectContaining({ automaticEnabled: true, intervalMinutes: 15 }) });
  await user.click(screen.getByRole("button", { name: "关闭" }));
  expect(close).toHaveBeenCalledOnce();
});

it("restores mapping focus on Escape and preserves links during telemetry refresh", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  function ControlledMapping({ refresh }: { refresh: number }) {
    const [open, setOpen] = useState(false);
    return <UiLocaleProvider>
      <button onClick={() => setOpen(true)}>Open mapping</button>
      <ProviderModelMappingDialog open={open} providerId="p" providerLabel="Pool" locked={false}
        modelOptions={refresh ? ["other", "a"] : ["a", "other"]} upstreamModelOptions={["b", "c"]}
        initialEntries={[]} onOpenChange={setOpen} onSubmit={onSubmit} />
    </UiLocaleProvider>;
  }
  const view = render(<ControlledMapping refresh={0} />);
  const trigger = screen.getByRole("button", { name: "Open mapping" });
  await user.click(trigger);
  await user.click(screen.getByRole("checkbox", { name: "b" }));
  view.rerender(<ControlledMapping refresh={1} />);
  expect(screen.getByRole("checkbox", { name: "b" })).toBeChecked();
  expect(onSubmit).toHaveBeenCalledWith([{ model: "a", upstreamModel: "b" }]);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(trigger).toHaveFocus());
});

it("shows mapping targets, filters mapped sources and searches targets without dropping hidden selections", async () => {
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderModelMappingDialog open providerId="p" providerLabel="Pool"
    locked={false} modelOptions={["a", "other"]} upstreamModelOptions={["b", "c"]}
    initialEntries={[{ model: "a", upstreamModel: "b" }]} onOpenChange={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>);
  expect(screen.queryByText(/未关联时原名直传|左侧按本机调用量排序/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "a" })).toHaveTextContent("→ b");
  await user.click(screen.getByRole("checkbox", { name: "已映射" }));
  expect(screen.queryByRole("button", { name: "other" })).not.toBeInTheDocument();
  await user.type(screen.getByRole("textbox", { name: "搜索凭据池模型" }), "c");
  expect(screen.queryByRole("checkbox", { name: "b" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("checkbox", { name: "c" }));
  expect(screen.getByText("已选 2")).toBeInTheDocument();
  expect(onSubmit).toHaveBeenCalledWith([{ model: "a", upstreamModel: "b" }, { model: "a", upstreamModel: "c" }]);
  await user.click(screen.getByRole("button", { name: "删除映射模型" }));
  expect(screen.getByText("已选 0")).toBeInTheDocument();
});