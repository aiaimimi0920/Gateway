import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaOAuthPanel } from "./LumaOAuthPanel";
import type { LumaOAuthProps } from "./useLumaOAuth";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const binding = { revision: "r1", providerId: "mcp", credentialId: "saved" };
const model = { id: "sample", modality: "image", aliases: [], actions: [], capabilities: {}, pricing: null, strengths: null };
const list = { ...binding, result: { models: [model], source: "registry" } };
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(list));
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "r1", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { request, props };
}
const click = async (name: string) => { await act(async () => { fireEvent.click(screen.getByRole("button", { name })); }); };
it("reads models and selected options explicitly without generating or changing the account", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />);
  expect(f.request).not.toHaveBeenCalled(); await click("读取模型");
  expect(f.request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/luma-mcp-models", expect.anything(), expect.objectContaining({
    body: { providerId: "mcp", credentialId: "saved", expectedRevision: "r1", action: "list" } }));
  expect(screen.getByLabelText("可选模型")).toHaveValue("sample");
  f.request.mockImplementationOnce(async (_path, schema) => schema.parse({ ...binding, result: {
    model: "sample", type: "image", parameters: { type: "object", properties: { quality: { enum: ["standard"] } } }, media_roles: [], guidance: null } }));
  await click("读取模型参数");
  expect(screen.getByLabelText("模型参数")).toHaveValue(JSON.stringify({ type: "object", properties: { quality: { enum: ["standard"] } } }, null, 2));
  expect(f.request).toHaveBeenLastCalledWith("/v1/internal/gateway/console/luma-mcp-models", expect.anything(), expect.objectContaining({
    body: { providerId: "mcp", credentialId: "saved", expectedRevision: "r1", action: "describe", model: "sample", type: "image" } }));
  expect(f.props.onSaved).not.toHaveBeenCalled();
});
it("requires secret consent and handles a rejected consent prompt", async () => {
  const f = fixture(); f.props.onRequestSecretAccess = vi.fn().mockRejectedValue(new Error("private"));
  render(<LumaOAuthPanel {...f.props} secretGrant={null} />); await click("读取模型");
  expect(f.request).not.toHaveBeenCalled(); expect(screen.queryByText(/private/)).not.toBeInTheDocument();
});
it.each(["expectedRevision", "managementToken", "credentialId", "providerId", "secretGrant"] as const)("discards late models when %s changes", async (key) => {
  const f = fixture(); let resolve!: (v: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise((done) => { resolve = done; }));
  const view = render(<LumaOAuthPanel {...f.props} />); await click("读取模型");
  const signal = f.request.mock.calls[0][2]!.signal!;
  view.rerender(<LumaOAuthPanel {...f.props} {...{ [key]: "changed" }} />);
  expect(signal.aborted).toBe(true); await act(async () => resolve(list));
  expect(screen.queryByLabelText("可选模型")).not.toBeInTheDocument();
});
it("does not expose errors or accept a response from another account", async () => {
  const f = fixture(); f.request.mockResolvedValueOnce({ ...list, credentialId: "other" });
  render(<LumaOAuthPanel {...f.props} />); await click("读取模型");
  expect(screen.queryByLabelText("可选模型")).not.toBeInTheDocument();
  f.request.mockRejectedValueOnce(new Error("secret upstream")); await click("读取模型");
  expect(screen.queryByText(/secret upstream/)).not.toBeInTheDocument();
});
it("shows unknown rates without claiming free generation and rejects mismatched model details", async () => {
  const f = fixture(); render(<LumaOAuthPanel {...f.props} />); await click("读取模型");
  expect(screen.getByLabelText("公布费率")).toHaveValue("未知");
  f.request.mockResolvedValueOnce({ ...binding, result: { model: "other", type: "image", parameters: {}, media_roles: [], guidance: null } });
  await click("读取模型参数"); expect(screen.queryByLabelText("模型参数")).not.toBeInTheDocument();
  expect(screen.getByText(/模型读取失败/)).toBeInTheDocument();
});
it("stops a pending read without retrying and aborts on unmount", async () => {
  const f = fixture(); f.request.mockImplementation(() => new Promise(() => {}));
  const view = render(<LumaOAuthPanel {...f.props} />); await click("读取模型");
  expect(screen.getByRole("button", { name: "读取模型" })).toBeDisabled();
  await click("停止读取模型"); expect(f.request.mock.calls[0][2]!.signal!.aborted).toBe(true);
  expect(f.request).toHaveBeenCalledOnce(); await click("读取模型"); view.unmount();
  expect(f.request.mock.calls[1][2]!.signal!.aborted).toBe(true);
});
it("does not automatically read when consent arrives and never renders upstream markup", async () => {
  const f = fixture(); const view = render(<LumaOAuthPanel {...f.props} secretGrant={null} />);
  await click("读取模型"); view.rerender(<LumaOAuthPanel {...f.props} />);
  expect(f.request).not.toHaveBeenCalled();
  f.request.mockResolvedValueOnce({ ...binding, result: { source: "registry", models: [{ ...model, id: "<script>bad</script>" }] } });
  await click("读取模型"); expect(view.container.querySelector("script")).toBeNull();
  expect(screen.getByLabelText("可选模型")).toHaveValue("<script>bad</script>");
});
it("reports an empty list without inventing a default model", async () => {
  const f = fixture(); f.request.mockResolvedValueOnce({ ...binding, result: { source: "registry", models: [] } });
  render(<LumaOAuthPanel {...f.props} />); await click("读取模型");
  expect(screen.getByText("模型列表为空")).toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "读取模型参数" })).not.toBeInTheDocument();
});
