import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { LumaMcpCreationPanel } from "./LumaMcpCreationPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const task = { id: "luma-mcp-existing", providerId: "mcp", credentialId: "saved", revision: "original",
  kind: "image", budgetCredits: 20, state: "running", sessionId: "s1", runId: "r1", ended: false,
  leaseUntil: 0, error: null, assets: [], usage: null };
const response = { revision: "original", providerId: "mcp", credentialId: "saved", task };
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "original", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { request, props };
}
const click = async (name: string) => { await act(async () => fireEvent.click(screen.getByRole("button", { name }))); };
const identify = () => fireEvent.change(screen.getByLabelText("任务 ID"), { target: { value: task.id } });

it("keeps the task ID on revision change and explicitly reads the original task under the current account", async () => {
  const f = fixture(); const view = render(<LumaMcpCreationPanel {...f.props} />);
  identify(); await click("查询结果与费用");
  expect(screen.getByText("生成中")).toBeInTheDocument();
  view.rerender(<LumaMcpCreationPanel {...f.props} expectedRevision="refreshed" />);
  expect(screen.getByLabelText("任务 ID")).toHaveValue(task.id);
  expect(screen.queryByText("生成中")).not.toBeInTheDocument();
  expect(f.request).toHaveBeenCalledTimes(1);
  f.request.mockResolvedValue({ ...response, revision: "refreshed" });
  await click("查询结果与费用"); await click("取消生成"); await click("结束会话");
  expect(f.request.mock.calls.slice(1).map(call => call[2]?.body)).toEqual(["read", "cancel", "end"].map(action => ({
    action, expectedRevision: "refreshed", providerId: "mcp", credentialId: "saved", taskId: task.id,
  })));
  expect(screen.queryByText("操作未确认。请保留任务 ID，查询原任务后再操作。")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "提交生成" })).not.toBeInTheDocument();
});

it("preserves the typed ID across secret consent but clears it on account change", async () => {
  const f = fixture(); const view = render(<LumaMcpCreationPanel {...f.props} secretGrant={null} />);
  identify(); await click("查询结果与费用");
  expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce();
  expect(f.request).not.toHaveBeenCalled();
  view.rerender(<LumaMcpCreationPanel {...f.props} />);
  expect(screen.getByLabelText("任务 ID")).toHaveValue(task.id);
  expect(f.request).not.toHaveBeenCalled(); await click("查询结果与费用");
  view.rerender(<LumaMcpCreationPanel {...f.props} credentialId="different" />);
  expect(screen.getByLabelText("任务 ID")).toHaveValue("");
  expect(screen.queryByText("生成中")).not.toBeInTheDocument();
});

it("rejects stale envelope revisions and foreign task bindings while retaining the requested ID", async () => {
  for (const value of [response, { ...response, revision: "current", task: { ...task, credentialId: "different" } },
    { ...response, revision: "current", task: { ...task, id: "different-task" } }]) {
    const f = fixture(); f.request.mockResolvedValue(value);
    const view = render(<LumaMcpCreationPanel {...f.props} expectedRevision="current" />);
    identify(); await click("查询结果与费用");
    expect(screen.getByText("操作未确认。请保留任务 ID，查询原任务后再操作。")).toBeInTheDocument();
    expect(screen.getByLabelText("任务 ID")).toHaveValue(task.id);
    expect(screen.queryByText("生成中")).not.toBeInTheDocument(); view.unmount();
  }
});

it("discards an in-flight result after a configuration update without automatic retry", async () => {
  const f = fixture(); let finish!: (value: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  const view = render(<LumaMcpCreationPanel {...f.props} />);
  identify(); await click("查询结果与费用");
  view.rerender(<LumaMcpCreationPanel {...f.props} expectedRevision="current" />);
  expect(f.request.mock.calls[0][2]?.signal?.aborted).toBe(true);
  await act(async () => finish(response));
  expect(screen.getByLabelText("任务 ID")).toHaveValue(task.id);
  expect(screen.queryByText("生成中")).not.toBeInTheDocument();
  expect(f.request).toHaveBeenCalledTimes(1);
});
