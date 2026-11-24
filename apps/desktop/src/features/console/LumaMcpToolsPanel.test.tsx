import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaOAuthPanel } from "./LumaOAuthPanel";
import type { LumaOAuthProps } from "./useLumaOAuth";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const tool = { name: "luma_model_list", title: null, description: "<script>untrusted</script>",
  inputSchema: { type: "object", properties: {} }, outputSchema: null };
const response = { revision: "r1", providerId: "mcp", credentialId: "saved", tools: [tool] };
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "r1", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { request, props };
}
const read = async () => { await act(async () => { fireEvent.click(screen.getByRole("button", { name: "读取工具目录" })); }); };
it("reads tools only on explicit action from the saved-account authorization entry", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} />);
  expect(f.request).not.toHaveBeenCalled(); await read();
  expect(f.request).toHaveBeenCalledTimes(1);
  expect(f.request).toHaveBeenCalledWith("/v1/internal/gateway/console/luma-mcp-tools", expect.anything(), expect.objectContaining({
    method: "POST", managementToken: "owner", secretGrant: "grant", cache: "no-store",
    body: { providerId: "mcp", credentialId: "saved", expectedRevision: "r1" } }));
  expect(screen.getByLabelText("工具")).toHaveValue("luma_model_list");
  expect(screen.getByLabelText("说明")).toHaveValue(tool.description);
  expect(view.container.querySelector("script")).toBeNull();
  expect(screen.queryByRole("button", { name: "执行工具" })).not.toBeInTheDocument();
  expect(f.props.onSaved).not.toHaveBeenCalled();
});
it("asks for consent without replaying the read when a grant arrives", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} secretGrant={null} />); await read();
  expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce(); expect(f.request).not.toHaveBeenCalled();
  view.rerender(<LumaOAuthPanel {...f.props} />); expect(f.request).not.toHaveBeenCalled();
  await read(); expect(f.request).toHaveBeenCalledOnce();
});
it("handles rejected consent without sending or exposing the rejection", async () => {
  const f = fixture(); f.props.onRequestSecretAccess = vi.fn().mockRejectedValue(new Error("private-consent-error"));
  render(<LumaOAuthPanel {...f.props} secretGrant={null} />); await read();
  expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce();
  expect(f.request).not.toHaveBeenCalled();
  expect(screen.getByText(/工具目录读取失败/)).toBeInTheDocument();
  expect(screen.queryByText(/private-consent-error/)).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "读取工具目录" })).toBeEnabled();
});
it("rejects mismatched response binding and never displays upstream error bodies", async () => {
  const f = fixture(); f.request.mockResolvedValueOnce({ ...response, credentialId: "other" });
  render(<LumaOAuthPanel {...f.props} />); await read();
  expect(screen.queryByLabelText("工具")).not.toBeInTheDocument();
  expect(screen.getByText(/工具目录读取失败/)).toBeInTheDocument();
  f.request.mockRejectedValueOnce(new Error("secret-upstream-body")); await read();
  expect(screen.queryByText(/secret-upstream-body/)).not.toBeInTheDocument();
});
it.each(["expectedRevision", "managementToken", "secretGrant", "providerId", "credentialId"] as const)("fences late results after %s changes", async (key) => {
  const f = fixture(); let resolve!: (value: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
  const view = render(<LumaOAuthPanel {...f.props} />); await read();
  const signal = f.request.mock.calls[0][2]!.signal!;
  view.rerender(<LumaOAuthPanel {...f.props} {...{ [key]: "changed" }} />);
  expect(signal.aborted).toBe(true);
  await act(async () => resolve(response)); expect(screen.queryByLabelText("工具")).not.toBeInTheDocument();
});
it("aborts on close and does not overlap reads or automatically retry", async () => {
  const f = fixture(); f.request.mockImplementationOnce(() => new Promise(() => {}));
  const view = render(<LumaOAuthPanel {...f.props} />); await read();
  expect(screen.getByRole("button", { name: "读取工具目录" })).toBeDisabled();
  expect(f.request).toHaveBeenCalledOnce(); view.unmount();
  expect(f.request.mock.calls[0][2]!.signal!.aborted).toBe(true);
});
it("requires a saved account and clean configuration", () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} credentialId={undefined} />);
  expect(screen.queryByRole("button", { name: "读取工具目录" })).not.toBeInTheDocument();
  view.rerender(<LumaOAuthPanel {...f.props} disabled />);
  expect(screen.getByRole("button", { name: "读取工具目录" })).toBeDisabled();
});
it("shows an empty catalog without inventing models or tools", async () => {
  const f = fixture(); f.request.mockResolvedValueOnce({ ...response, tools: [] });
  render(<LumaOAuthPanel {...f.props} />); await read();
  expect(screen.getByText("工具目录为空")).toBeInTheDocument();
  expect(screen.queryByLabelText("工具")).not.toBeInTheDocument();
});
it("discards completed results when the connection changes", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} />); await read();
  expect(screen.getByLabelText("工具")).toBeInTheDocument();
  view.rerender(<LumaOAuthPanel {...f.props} client={{ request: vi.fn() }} />);
  expect(screen.queryByLabelText("工具")).not.toBeInTheDocument();
});
