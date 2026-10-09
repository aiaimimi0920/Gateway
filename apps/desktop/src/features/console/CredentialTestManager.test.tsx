import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import type { ConsoleProviderProbeResponse, ConsoleRouteDocument } from "../../api/contracts";
import { CredentialTestManager } from "./CredentialTestManager";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import { updateTestPlans, type TestPlanChange } from "./credentialTestPlansDocument";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";

const document: ConsoleRouteDocument = { providers: [{ id: "p", label: "Pool", supported_models: ["a"], credentials: [{ id: "one", account_name: "One" }],
  test_plans: [{ id: "existing", name: "已保存计划", scopes: [{ kind: "pool" }], policy: defaultTestPolicy() }] }], model_routes: [], aliases: {}, account_groups: [] };
const response: ConsoleProviderProbeResponse = { result: { providerId: "p", status: "passed", message: "complete", checkedAt: "2026-10-05T00:00:00Z",
  totalCount: 1, passedCount: 1, failedCount: 0, unsupportedCount: 0, results: [{ providerId: "p", credentialId: "one", probePoint: "matrix",
    status: "passed", message: "complete", checkedAt: "2026-10-05T00:00:00Z", assessment: { planId: "existing", policySource: "plan:existing", mode: "manual",
      models: [{ model: "a", callable: true, completedCount: 1, gradedCount: 1, correctCount: 1, score: 100, capabilityLevel: "basic" }],
      cases: [{ caseId: "connection", name: "连接", model: "a", difficulty: 1, status: "passed", answer: "OK", expectedAnswer: "OK", correct: true, message: "done" }] } }] } };
function setup(empty = false, sharedResponse: ConsoleProviderProbeResponse | null = null) {
  const onProbe = vi.fn().mockResolvedValue(response);
  const onRead = vi.fn().mockResolvedValue(response);
  const changes = vi.fn(); const close = vi.fn();
  const section = pilotSection({ providerId: "p", providerIds: ["p"], providerLabel: "Pool" });
  const t = (zh: string) => zh;
  function Harness() {
    const [open, setOpen] = useState(false);
    const [doc, setDoc] = useState(empty ? { ...document, providers: [{ id: "p", supported_models: ["a"], credentials: [{ id: "one" }] }] } : document);
    return <><button onClick={() => setOpen(true)}>凭据池测试</button>{open ? <CredentialTestManager document={doc} initialTab="auto"
      onProbe={onProbe} onLoadResults={onRead} onSave={vi.fn().mockReturnValue(true)}
      onPlanChange={(updates: TestPlanChange[]) => { changes(updates); setDoc(updateTestPlans(doc, updates)); return true; }}
      probe={{ t, section, closePilotActionDialog: () => { close(); setOpen(false); }, providerProbeBusy: false, providerProbeError: null,
        providerProbeResponse: sharedResponse, draftDirty: false, draftMatchesActiveRevision: true }}
      schedule={{ t, section, closePilotActionDialog: close, editorLocked: false, pilotScheduleEnabled: false, setPilotScheduleEnabled: vi.fn(),
        pilotScheduleIntervalMinutes: "60", setPilotScheduleIntervalMinutes: vi.fn(), applyProviderProbeSchedule: vi.fn() }} /> : null}</>;
  }
  render(<Harness />); return { onProbe, onRead, changes, close };
}

it("opens the list, expands its own result, and switches to model summary without calls", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  expect(screen.getByRole("tab", { name: "测试" })).toHaveAttribute("aria-selected", "true");
  expect(screen.queryByRole("textbox", { name: "计划名称" })).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "已保存计划" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "展开测试结果 · 已保存计划" }));
  await waitFor(() => expect(p.onRead).toHaveBeenCalledWith("p", undefined, { planId: "existing" }));
  expect(await screen.findByText("答案详情")).toBeInTheDocument();
  await user.click(screen.getByRole("tab", { name: "IQ" }));
  expect(screen.getByRole("region", { name: "模型 IQ 看板" })).toBeInTheDocument();
  expect(screen.getByText("测试 IQ")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: /^a$/ })).toBeInTheDocument();
  expect(p.onProbe).not.toHaveBeenCalled();
  await user.keyboard("{Escape}");
  expect(p.close).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "凭据池测试" })).toHaveFocus();
});

it("adds, edits with a stable ID, returns to the list and confirms deletion", async () => {
  const user = userEvent.setup(); const p = setup(true);
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  expect(screen.getByText("暂无测试计划")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  const name = screen.getByRole("textbox", { name: "计划名称" });
  expect(name).toHaveFocus();
  await user.type(name, "新计划");
  expect(screen.queryByRole("button", { name: "运行测试" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(screen.getByRole("heading", { name: "新计划" })).toBeInTheDocument();
  const id = p.changes.mock.calls[0][0][0].id;
  await user.click(screen.getByRole("button", { name: "编辑" }));
  await user.clear(screen.getByRole("textbox", { name: "计划名称" }));
  await user.type(screen.getByRole("textbox", { name: "计划名称" }), "改名计划");
  await user.click(screen.getByRole("button", { name: "保存计划" }));
  expect(p.changes.mock.calls[1][0][0]).toMatchObject({ id, plan: { id, name: "改名计划" } });
  await user.click(screen.getByRole("button", { name: "删除" }));
  const confirm = screen.getByRole("alertdialog", { name: "删除测试计划" });
  await user.click(within(confirm).getByRole("button", { name: "取消" }));
  expect(p.changes).toHaveBeenCalledTimes(2);
  await user.click(screen.getByRole("button", { name: "删除" }));
  await user.click(screen.getByRole("button", { name: "确认删除" }));
  expect(screen.getByText("暂无测试计划")).toBeInTheDocument();
  expect(p.changes.mock.calls[2][0]).toEqual([{ providerId: "p", id, plan: null }]);
  expect(p.onProbe).not.toHaveBeenCalled();
});

it("runs the saved plan by ID and never substitutes a temporary or inherited policy", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  await user.click(screen.getByRole("button", { name: "手动测试" }));
  await waitFor(() => expect(p.onProbe).toHaveBeenCalledTimes(1));
  await user.click(screen.getByRole("button", { name: "编辑" }));
  expect(screen.queryByRole("checkbox", { name: "继承" })).not.toBeInTheDocument();
  expect(p.onProbe).toHaveBeenCalledExactlyOnceWith({ planId: "existing", credentialIds: ["one"] }, "p");
  await user.click(screen.getByRole("button", { name: "返回列表" }));
  expect(screen.getByRole("button", { name: "编辑" })).toHaveFocus();
  expect(p.onProbe).toHaveBeenCalledTimes(1);
});

it("keeps top-level tab keyboard focus when leaving the editor", async () => {
  const user = userEvent.setup(); setup();
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  await user.click(screen.getByRole("button", { name: "编辑" }));
  const tests = screen.getByRole("tab", { name: "测试" });
  expect(window.document.getElementById(tests.getAttribute("aria-controls")!)).toBeInTheDocument();
  tests.focus(); await user.keyboard("{ArrowRight}");
  expect(screen.getByRole("tab", { name: "IQ" })).toHaveFocus();
  expect(screen.getByText("测试 IQ")).toBeInTheDocument();
});

it("never shows another plan's shared result in a new editor", async () => {
  const user = userEvent.setup(); const p = setup(false, response);
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  await user.click(screen.getByRole("button", { name: "展开测试结果 · 已保存计划" }));
  expect(await screen.findByRole("heading", { name: "One" })).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(screen.queryByText("暂无结果")).not.toBeInTheDocument();
  expect(screen.queryByRole("heading", { name: "One" })).not.toBeInTheDocument();
  expect(p.onProbe).not.toHaveBeenCalled();
});

it("shows the pool title and seven columns, with a configuration-only editor", async () => {
  const user = userEvent.setup(); const p = setup();
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  expect(screen.getByRole("heading", { name: "Pool" })).toBeInTheDocument();
  for (const name of ["名字", "测试范围", "测试集", "自动测试间隔", "手动测试", "编辑", "删除"])
    expect(screen.getByRole("columnheader", { name })).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "刷新结果" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "编辑" }));
  expect(screen.queryByRole("tab", { name: "自动测试" })).not.toBeInTheDocument();
  expect(screen.queryByRole("tab", { name: "手动测试" })).not.toBeInTheDocument();
  expect(screen.queryByText(/^测试账户/)).not.toBeInTheDocument();
  expect(screen.queryByText("暂无结果")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "手动测试" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存计划" })).toBeInTheDocument();
  expect(p.onProbe).not.toHaveBeenCalled();
});

it("keeps result reads silent while opening and expanding an empty plan", async () => {
  const user = userEvent.setup(); const p = setup();
  let finish!: (value: ConsoleProviderProbeResponse) => void;
  p.onRead.mockImplementation(() => new Promise<ConsoleProviderProbeResponse>((resolve) => { finish = resolve; }));
  await user.click(screen.getByRole("button", { name: "凭据池测试" }));
  expect(screen.queryByText("读取结果…")).not.toBeInTheDocument();
  await act(async () => { finish({ result: { ...response.result, results: [] } }); });
  await user.click(screen.getByRole("button", { name: "展开测试结果 · 已保存计划" }));
  expect(p.onRead).toHaveBeenLastCalledWith("p", undefined, { planId: "existing" });
  expect(screen.queryByText("读取结果…")).not.toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "已保存计划" })).toBeInTheDocument();
  await act(async () => { finish(response); });
  expect(await screen.findByText("答案详情")).toBeInTheDocument();
});
