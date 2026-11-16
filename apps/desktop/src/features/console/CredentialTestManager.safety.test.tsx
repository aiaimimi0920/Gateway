import { NeuroProvider } from "@neuro/ui";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import type { ConsoleProviderProbeResponse, ConsoleRouteDocument } from "../../api/contracts";
import { CredentialTestManager } from "./CredentialTestManager";
import { CredentialTestDialog } from "./CredentialTestDialog";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import { updateTestPlans } from "./credentialTestPlansDocument";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";

const doc: ConsoleRouteDocument = {
  providers: [{
    id: "p", label: "Pool", supported_models: ["a"], credentials: [{ id: "one" }],
    test_plans: [{ id: "saved", name: "Saved", scopes: [{ kind: "pool" }], policy: defaultTestPolicy() }]
  }], model_routes: [], aliases: {}, account_groups: []
};
const response: ConsoleProviderProbeResponse = {
  result: {
    providerId: "p", status: "passed", message: "done", checkedAt: "2026-11-04T00:00:00Z",
    totalCount: 1, passedCount: 1, failedCount: 0, unsupportedCount: 0, results: []
  }
};

function setup(manager = true, multi = false) {
  const change = vi.fn().mockReturnValue(true); const close = vi.fn();
  const probe = vi.fn().mockResolvedValue(response); const load = vi.fn().mockResolvedValue(response);
  let lock!: (value: boolean) => void;
  const section = pilotSection({ providerId: "p", providerIds: multi ? ["p", "q"] : ["p"], providerLabel: "Pool" });
  const t = (zh: string) => zh;
  function Harness() {
    const [open, setOpen] = useState(false); const [locked, setLocked] = useState(false); lock = setLocked;
    const [document, setDocument] = useState<ConsoleRouteDocument>(multi ? { ...doc, providers: [...doc.providers, { id: "q", supported_models: ["a"], credentials: [{ id: "two" }] }] } : doc);
    const Component = manager ? CredentialTestManager : CredentialTestDialog;
    const dismiss = () => { close(); setOpen(false); };
    return <><button onClick={() => setOpen(true)}>Open tests</button>{open ? <Component document={document} initialTab="auto"
      onProbe={probe} onLoadResults={load} onPlanChange={(changes) => { if (!change(changes)) return false; setDocument(updateTestPlans(document, changes)); return true; }}
      probe={{
        t, section, closePilotActionDialog: dismiss, providerProbeBusy: false, providerProbeError: null,
        providerProbeResponse: null, draftDirty: false, draftMatchesActiveRevision: true
      }}
      schedule={{
        t, section, closePilotActionDialog: dismiss, editorLocked: locked, pilotScheduleEnabled: false,
        setPilotScheduleEnabled: vi.fn(), pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn()
      }} /> : null}</>;
  }
  render(<Harness />, { wrapper: NeuroProvider });
  return { change, close, probe, load, lock: (value: boolean) => act(() => lock(value)) };
}

it("shows deletion failures inside the confirmation, retains cancel-first focus and respects revoked permissions", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "Open tests" }));
  const remove = screen.getByRole("button", { name: "删除" });
  await user.click(remove);
  const confirm = screen.getByRole("alertdialog", { name: "删除测试计划" });
  expect(within(confirm).getByRole("button", { name: "取消" })).toHaveFocus();
  p.change.mockReturnValue(false);
  await user.click(within(confirm).getByRole("button", { name: "确认删除" }));
  expect(within(confirm).getByRole("alert")).toHaveTextContent("计划删除失败");
  expect(confirm).toBeInTheDocument();
  p.lock(true);
  expect(within(confirm).getByRole("button", { name: "确认删除" })).toBeDisabled();
  expect(within(confirm).getByRole("button", { name: "取消" })).toBeEnabled();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole("button", { name: "关闭测试弹窗" })).toHaveFocus());
  expect(p.close).not.toHaveBeenCalled(); expect(p.change).toHaveBeenCalledTimes(1);
  p.lock(false);
  await user.click(remove); await user.keyboard("{Escape}");
  expect(remove).toHaveFocus();
});

it("returns to Add when successful deletion removes the confirmation trigger", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "Open tests" }));
  await user.click(screen.getByRole("button", { name: "删除" }));
  await user.click(screen.getByRole("button", { name: "确认删除" }));
  expect(screen.getByText("暂无测试计划")).toBeInTheDocument();
  await waitFor(() => expect(screen.getByRole("button", { name: "添加" })).toHaveFocus());
  expect(p.change).toHaveBeenCalledExactlyOnceWith([{ providerId: "p", id: "saved", plan: null }]);
});

it("leaves the editor open on save failure without executing the plan", async () => {
  const user = userEvent.setup(); const p = setup(); p.change.mockReturnValue(false);
  await user.click(screen.getByRole("button", { name: "Open tests" }));
  await user.click(screen.getByRole("button", { name: "编辑" }));
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(screen.getByRole("textbox", { name: "计划名称" })).toHaveValue("Saved");
  expect(screen.getByRole("alert")).toHaveTextContent("计划保存失败");
  expect(p.probe).not.toHaveBeenCalled();
});

it("allows closing during execution and ignores the late response before sending another pool", async () => {
  const user = userEvent.setup(); const p = setup(false, true);
  let finish!: (value: ConsoleProviderProbeResponse) => void;
  p.probe.mockImplementation(() => new Promise<ConsoleProviderProbeResponse>((resolve) => { finish = resolve; }));
  const trigger = screen.getByRole("button", { name: "Open tests" });
  await user.click(trigger); await user.click(screen.getByRole("button", { name: "运行测试" }));
  expect(p.probe).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("button", { name: "关闭测试弹窗" })).toBeEnabled();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  await act(async () => finish(response));
  expect(p.probe).toHaveBeenCalledTimes(1); expect(p.close).toHaveBeenCalledOnce();
  expect(p.load).not.toHaveBeenCalled(); expect(trigger).toHaveFocus();
});
