import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { LumaMcpCreationPanel } from "./LumaMcpCreationPanel";
import { OperationsRequestTable } from "./OperationsRequestTable";
import { operationsRequestFixture } from "./OperationsWorkspace.fixtures";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const task = { id: "luma-mcp-test", providerId: "mcp", credentialId: "saved", revision: "r1", kind: "image",
  budgetCredits: 20, state: "prepared", sessionId: null, runId: null, ended: false, leaseUntil: 0, error: null, assets: [], usage: null };
const response = { revision: "r1", providerId: "mcp", credentialId: "saved", task };
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "r1", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { request, props };
}
const click = async (name: string) => { await act(async () => fireEvent.click(screen.getByRole("button", { name }))); };
const fill = () => {
  fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "blue circle" } });
  fireEvent.change(screen.getByLabelText("会话预算上限（credits）"), { target: { value: "20" } });
};
it("prepares without spending and requires explicit confirmation before the single submit", async () => {
  const f = fixture(); render(<LumaMcpCreationPanel {...f.props} />);
  expect(f.request).not.toHaveBeenCalled(); fill(); await click("准备任务");
  expect(f.request.mock.calls[0][2]?.body).toMatchObject({ action: "prepare", request: { kind: "image", prompt: "blue circle", budgetCredits: 20 } });
  expect(screen.getByRole("button", { name: "提交生成" })).toBeDisabled();
  fireEvent.click(screen.getByLabelText("确认使用此 Luma 账号的 credits 生成"));
  f.request.mockResolvedValueOnce({ ...response, task: { ...task, state: "running", runId: "r1", sessionId: "s1" } });
  await click("提交生成");
  expect(f.request.mock.calls[1][2]?.body).toMatchObject({ action: "start", taskId: task.id, chargeConfirmed: true });
  expect(screen.queryByRole("button", { name: "提交生成" })).not.toBeInTheDocument();
  f.request.mockResolvedValueOnce({ ...response, task: { ...task, state: "completed", runId: "r1", sessionId: "s1",
    assets: [{ id: "a1", kind: "image", state: "ready", url: "https://cdn.example.com/a.png" }],
    usage: { credits: 9, settled: false, estimated: true, unresolvedEvents: 0, scope: "session" } } });
  await click("查询结果与费用");
  expect(f.request.mock.calls[2][2]?.body).not.toHaveProperty("request");
  expect(screen.getByRole("link", { name: "打开生成结果" })).toHaveAttribute("href", "https://cdn.example.com/a.png");
  expect(screen.getByText(/9 credits · 未结算/)).toBeInTheDocument();
});
it("keeps a known task after an uncertain submit and never offers an automatic generation retry", async () => {
  const f = fixture(); render(<LumaMcpCreationPanel {...f.props} />); fill(); await click("准备任务");
  fireEvent.click(screen.getByLabelText("确认使用此 Luma 账号的 credits 生成"));
  f.request.mockRejectedValueOnce(new Error("private-token")); await click("提交生成");
  expect(screen.queryByText(/private-token/)).not.toBeInTheDocument();
  expect(screen.getByLabelText("任务 ID")).toHaveValue(task.id);
  expect(screen.queryByRole("button", { name: "提交生成" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "查询结果与费用" })).toBeEnabled();
  expect(f.request).toHaveBeenCalledTimes(2);
});
it("separates run cancellation and session ending", async () => {
  const f = fixture(); f.request.mockResolvedValue({ ...response, task: { ...task, state: "running", runId: "r1", sessionId: "s1" } });
  render(<LumaMcpCreationPanel {...f.props} />);
  fireEvent.change(screen.getByLabelText("任务 ID"), { target: { value: task.id } });
  await click("查询结果与费用"); await click("取消生成"); await click("结束会话");
  expect(f.request.mock.calls.map(call => (call[2]?.body as { action: string }).action)).toEqual(["read", "cancel", "end"]);
});
it("fences late account results and does not replay after consent", async () => {
  const f = fixture(); const view = render(<LumaMcpCreationPanel {...f.props} secretGrant={null} />);
  fill(); await click("准备任务"); expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce();
  expect(f.request).not.toHaveBeenCalled(); view.rerender(<LumaMcpCreationPanel {...f.props} />);
  let resolve!: (value: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise(done => { resolve = done; })); await click("准备任务");
  view.rerender(<LumaMcpCreationPanel {...f.props} credentialId="other" />);
  expect(f.request.mock.calls[0][2]?.signal?.aborted).toBe(true);
  await act(async () => resolve(response)); expect(screen.getByLabelText("任务 ID")).toHaveValue("");
});
it("recovers the task ID and unknown costs from the normal operations ledger", () => {
  const row = { ...operationsRequestFixture(), status: "pending", routeTrace: { lumaMcpCreation: task } };
  render(<OperationsRequestTable rows={[row]} t={zh => zh} />);
  fireEvent.click(screen.getByRole("button", { name: `请求详情: ${row.id}` }));
  expect(screen.getByRole("button", { name: "复制 MCP 任务 ID" })).toBeInTheDocument();
  expect(screen.getByText(task.id)).toBeInTheDocument();
  expect(screen.getByText("会话费用：未知")).toBeInTheDocument();
});
