import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { LumaMcpToolsPanel } from "./LumaMcpToolsPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const definition = { name: "luma_generate_image", title: "合成合同", description: "<script>untrusted</script>",
  inputSchema: { type: "object", $defs: { prompt: { type: "string" } }, properties: { prompt: { $ref: "#/$defs/prompt" } }, required: ["prompt"] },
  outputSchema: { type: "object", properties: { run_id: { type: "string" } } },
  annotations: { readOnlyHint: false, idempotentHint: false, futureHint: { label: "原样保留" } },
  _meta: { private: "excluded-tool-metadata" } };
const response = { revision: "r1", providerId: "mcp", credentialId: "saved", tools: [definition], private: "excluded-envelope" };
const read = async () => { await act(async () => fireEvent.click(screen.getByRole("button", { name: "读取工具目录" }))); };
const save = () => fireEvent.click(screen.getByRole("button", { name: "导出工具目录" }));
const saveButton = () => screen.getByRole("button", { name: "导出工具目录" });
function fixture() {
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const create = vi.fn((_blob: Blob) => "blob:catalog");
  const revoke = vi.fn();
  const click = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});
  vi.stubGlobal("URL", class extends URL { static createObjectURL = create; static revokeObjectURL = revoke; });
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "private-owner", secretGrant: "private-grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "r1", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  return { props, request, create, revoke, click };
}
function blobText(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader(); reader.onload = () => resolve(String(reader.result)); reader.onerror = reject;
    reader.readAsText(blob);
  });
}
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); vi.unstubAllGlobals(); });

it("downloads all contract fields from the completed read, without identities or extra requests", async () => {
  const f = fixture();
  const second = { ...definition, name: "luma_model_list", annotations: null };
  f.request.mockImplementationOnce(async (_path, schema) => schema.parse({ ...response, tools: [definition, second] }));
  const view = render(<LumaMcpToolsPanel {...f.props} />);
  expect(saveButton()).toBeDisabled(); save(); expect(f.create).not.toHaveBeenCalled();
  const before = Date.now(); await read(); const after = Date.now();
  expect(f.create).not.toHaveBeenCalled();
  vi.useFakeTimers(); save();
  const blob = f.create.mock.calls[0][0]; expect(blob.type).toBe("application/json;charset=utf-8");
  const anchor = f.click.mock.instances[0] as HTMLAnchorElement;
  expect(anchor.download).toMatch(/^luma-mcp-catalog-\d{4}-.*\.json$/);
  expect(anchor.isConnected).toBe(false);
  expect(f.revoke).not.toHaveBeenCalled(); vi.advanceTimersByTime(1000);
  expect(f.revoke).toHaveBeenCalledWith("blob:catalog"); vi.useRealTimers();
  const snapshot = JSON.parse(await blobText(blob));
  expect(snapshot).toMatchObject({ format: "gateway-luma-mcp-catalog", schemaVersion: 1,
    resource: "https://agents.lumalabs.ai/mcp", protocolVersion: "2025-03-26" });
  expect(Date.parse(snapshot.capturedAt)).toBeGreaterThanOrEqual(before);
  expect(Date.parse(snapshot.capturedAt)).toBeLessThanOrEqual(after);
  const { _meta, ...expected } = definition;
  expect(snapshot.tools).toEqual([expected, { ...expected, name: "luma_model_list", annotations: null }]);
  for (const secret of ["private-owner", "private-grant", "excluded-tool-metadata", "excluded-envelope", '"credentialId"', '"revision"', '"providerId"']) {
    expect(JSON.stringify(snapshot)).not.toContain(secret);
  }
  save(); expect(f.request).toHaveBeenCalledOnce();
  expect(await blobText(f.create.mock.calls[1][0])).toEqual(await blobText(blob));
  expect(view.container.querySelector("script")).toBeNull(); expect(f.props.onSaved).not.toHaveBeenCalled();
});

it.each(["expectedRevision", "managementToken", "secretGrant", "providerId", "credentialId", "disabled", "client"] as const)("invalidates completed export when %s changes", async key => {
  const f = fixture(); const view = render(<LumaMcpToolsPanel {...f.props} />); await read(); expect(saveButton()).toBeEnabled();
  const value = key === "disabled" ? true : key === "client" ? { request: vi.fn() } : "changed";
  view.rerender(<LumaMcpToolsPanel {...f.props} {...{ [key]: value }} />);
  expect(saveButton()).toBeDisabled(); save(); expect(f.create).not.toHaveBeenCalled();
});

it("discards the prior export on reread, cancellation and a late response", async () => {
  const f = fixture(); render(<LumaMcpToolsPanel {...f.props} />); await read();
  let resolve!: (value: unknown) => void;
  f.request.mockImplementationOnce(() => new Promise(done => { resolve = done; })); await read();
  expect(saveButton()).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "停止读取" }));
  await act(async () => resolve(response)); expect(saveButton()).toBeDisabled();
  f.request.mockRejectedValueOnce(new Error("private-read-failure")); await read();
  expect(saveButton()).toBeDisabled(); save(); expect(f.create).not.toHaveBeenCalled();
});

it("keeps missing annotations unknown and can export a genuinely empty catalog", async () => {
  const f = fixture(); const { annotations, _meta, ...legacy } = definition;
  f.request.mockResolvedValueOnce({ ...response, tools: [legacy] });
  render(<LumaMcpToolsPanel {...f.props} />); await read(); save();
  expect(JSON.parse(await blobText(f.create.mock.calls[0][0])).tools[0].annotations).toBeNull();
  f.request.mockResolvedValueOnce({ ...response, tools: [] }); await read(); save();
  expect(JSON.parse(await blobText(f.create.mock.calls[1][0])).tools).toEqual([]);
});

it("rejects invalid annotations and hides download exceptions without rereading", async () => {
  const f = fixture(); f.request.mockImplementationOnce(async (_path, schema) => schema.parse({ ...response, tools: [{ ...definition, annotations: [] }] }));
  render(<LumaMcpToolsPanel {...f.props} />); await read(); expect(saveButton()).toBeDisabled();
  await read(); f.click.mockImplementationOnce(() => { throw new Error("private-download-error"); });
  vi.useFakeTimers(); save(); expect(document.querySelector('a[download]')).toBeNull();
  vi.advanceTimersByTime(1000); expect(f.revoke).toHaveBeenCalledWith("blob:catalog"); vi.useRealTimers();
  expect(screen.getByText("工具目录导出失败，请重试。")).toBeInTheDocument();
  expect(screen.queryByText(/private-download-error/)).not.toBeInTheDocument();
  save(); expect(screen.queryByText("工具目录导出失败，请重试。")).not.toBeInTheDocument();
  expect(f.request).toHaveBeenCalledTimes(2);
});
