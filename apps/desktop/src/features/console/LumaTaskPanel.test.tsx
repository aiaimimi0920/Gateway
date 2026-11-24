import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaTaskPanel } from "./LumaTaskPanel";
import type { LumaFilesProps } from "./useLumaFiles";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const id = "d290f1ee-6c54-4b01-90e6-d701748f0851";
const response = { revision: "r1", providerId: "luma", credentialId: "exact", result: {
  id, model: "uni-1", type: "image", state: "completed", usage: { credits_charged: null } } };
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const props: LumaFilesProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "luma", credentialId: "exact", expectedRevision: "r1", disabled: false, onRequestSecretAccess: vi.fn() };
  return { request, props };
}
const enter = (value = id) => fireEvent.change(screen.getByLabelText("待查询任务 ID"), { target: { value } });
const read = async () => { await act(async () => { fireEvent.click(screen.getByRole("button", { name: "查询任务与费用" })); }); };
it("reads exactly once only on click, binding the saved account without login invalidation", async () => {
  const f = fixture(); render(<LumaTaskPanel {...f.props} />);
  expect(screen.getByRole("button", { name: "查询任务与费用" })).toBeDisabled();
  enter("../foreign"); expect(screen.getByRole("button", { name: "查询任务与费用" })).toBeDisabled();
  enter(id.toUpperCase()); expect(f.request).not.toHaveBeenCalled(); await read();
  expect(f.request).toHaveBeenCalledOnce();
  expect(f.request).toHaveBeenCalledWith("/v1/internal/gateway/console/luma-agents-task", expect.anything(), expect.objectContaining({
    method: "POST", cache: "no-store", managementToken: "owner", secretGrant: "grant", notifyAuthenticationFailure: false,
    body: { providerId: "luma", credentialId: "exact", expectedRevision: "r1", generationId: id } }));
  expect(screen.getByLabelText("Luma 任务 ID")).toHaveValue(id);
  expect(screen.getByText("未知")).toBeInTheDocument();
  expect(screen.queryByText("本地媒体验证")).not.toBeInTheDocument();
});
it.each([0, 12.5])("shows actual credits %s distinctly from unknown", async credits => {
  const f = fixture(); f.request.mockResolvedValue({ ...response, result: { ...response.result, usage: { credits_charged: credits } } });
  render(<LumaTaskPanel {...f.props} />); enter(); await read();
  expect(screen.getByText(String(credits))).toBeInTheDocument(); expect(screen.queryByText("未知")).not.toBeInTheDocument();
});
it("requests consent without automatic replay and handles a rejected consent", async () => {
  const f = fixture(); f.props.onRequestSecretAccess = vi.fn().mockRejectedValue(new Error("private-consent"));
  const view = render(<LumaTaskPanel {...f.props} secretGrant={null} />); enter(); await read();
  expect(f.request).not.toHaveBeenCalled(); expect(screen.getByText(/任务查询失败/)).toBeInTheDocument();
  expect(screen.queryByText(/private-consent/)).not.toBeInTheDocument();
  view.rerender(<LumaTaskPanel {...f.props} />); expect(f.request).not.toHaveBeenCalled();
});
it.each(["credentialId", "providerId", "expectedRevision", "managementToken", "secretGrant"] as const)("aborts and fences late results when %s changes", async key => {
  const f = fixture(); let resolve!: (value: unknown) => void;
  f.request.mockImplementation(() => new Promise(done => { resolve = done; }));
  const view = render(<LumaTaskPanel {...f.props} />); enter(); await read();
  const signal = f.request.mock.calls[0][2]!.signal!;
  view.rerender(<LumaTaskPanel {...f.props} {...{ [key]: "changed" }} />);
  expect(signal.aborted).toBe(true); expect(screen.getByLabelText("待查询任务 ID")).toHaveValue("");
  await act(async () => resolve(response)); expect(screen.queryByLabelText("Luma 任务 ID")).not.toBeInTheDocument();
});
it("changing task ID or closing cancels pending reads without overlap", async () => {
  const f = fixture(); f.request.mockImplementation(() => new Promise(() => {}));
  const view = render(<LumaTaskPanel {...f.props} />); enter(); await read();
  expect(screen.getByRole("button", { name: "查询任务与费用" })).toBeDisabled();
  enter("a290f1ee-6c54-4b01-90e6-d701748f0851"); expect(f.request.mock.calls[0][2]!.signal!.aborted).toBe(true);
  await read(); view.unmount(); expect(f.request.mock.calls[1][2]!.signal!.aborted).toBe(true);
});
it("rejects mismatched task identity and hides raw errors", async () => {
  const f = fixture(); f.request.mockResolvedValueOnce({ ...response, result: { ...response.result, id: "a290f1ee-6c54-4b01-90e6-d701748f0851" } });
  render(<LumaTaskPanel {...f.props} />); enter(); await read(); expect(screen.queryByLabelText("Luma 任务 ID")).not.toBeInTheDocument();
  f.request.mockRejectedValueOnce(new Error("private-upstream")); await read();
  expect(screen.getByText(/任务查询失败/)).toBeInTheDocument(); expect(screen.queryByText(/private-upstream/)).not.toBeInTheDocument();
});
it("stops a read and keeps disabled accounts inert", async () => {
  const f = fixture(); f.request.mockImplementation(() => new Promise(() => {}));
  const view = render(<LumaTaskPanel {...f.props} />); enter(); await read();
  fireEvent.click(screen.getByRole("button", { name: "停止查询" }));
  expect(f.request.mock.calls[0][2]!.signal!.aborted).toBe(true);
  view.rerender(<LumaTaskPanel {...f.props} disabled />); expect(screen.getByLabelText("待查询任务 ID")).toBeDisabled();
});
