import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import { LumaMcpCreationPanel } from "./LumaMcpCreationPanel";
import { mcpDraftOptions, newMcpDraft } from "./lumaMcpCreationDraft";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const sourceId = "luma-mcp-00112233-4455-6677-8899-aabbccddeeff";
const targetId = "luma-mcp-11112233-4455-6677-8899-aabbccddeeff";
const change = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } });
const click = async (name: string) => { await act(async () => fireEvent.click(screen.getByRole("button", { name }))); };
async function fixture(kind = "image") {
  const task = { id: sourceId, providerId: "mcp", credentialId: "saved", revision: "r1", kind,
    budgetCredits: 20, state: "completed", sessionId: "s1", runId: "run1", ended: true, leaseUntil: 0, error: null,
    assets: [{ id: "a1", kind, state: "ready", url: null }], usage: null };
  const response = { revision: "r1", providerId: "mcp", credentialId: "saved", task };
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const props = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "r1", disabled: false, onRequestSecretAccess: vi.fn() };
  const view = render(<LumaMcpCreationPanel {...props} />);
  change("任务 ID", sourceId); await click("查询结果与费用");
  return { request, props, view, response };
}
function fill() { change("提示词", "保留主体，改变背景"); change("会话预算上限（credits）", "13"); }

it("reuses a ready image without a public URL and asks for fresh consent and budget", async () => {
  const f = await fixture();
  await click("编辑此素材");
  expect(f.request).toHaveBeenCalledOnce();
  expect(screen.getByLabelText("素材来源任务 ID")).toHaveValue(sourceId);
  expect(screen.getByLabelText("源素材 ID")).toHaveValue("a1");
  expect(screen.getByLabelText("会话预算上限（credits）")).toHaveValue(null);
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  fill();
  f.request.mockResolvedValueOnce({ ...f.response, task: { ...f.response.task, id: targetId, state: "prepared", assets: [] } });
  await click("准备任务");
  const prepared = f.request.mock.calls[1][2]?.body as { request: unknown };
  expect(prepared).toMatchObject({ action: "prepare", request: { kind: "image", budgetCredits: 13,
    options: { model: "luma_uni_1", operation: "edit" },
    reuse: { taskId: sourceId, sourceAssetId: "a1", referenceAssetIds: [] } } });
  expect(screen.getByRole("button", { name: "提交生成" })).toBeDisabled();
  fireEvent.click(screen.getByLabelText("确认使用此 Luma 账号的 credits 生成"));
  f.request.mockResolvedValueOnce({ ...f.response, task: { ...f.response.task, id: targetId, state: "running" } });
  await click("提交生成");
  expect(f.request.mock.calls[2][2]?.body).toMatchObject({ action: "start", taskId: targetId, request: prepared.request, chargeConfirmed: true });
});

it("keeps the selected image as a GPT reference and clears reuse on account changes", async () => {
  const f = await fixture(); await click("用作参考图片");
  change("创作模型", "openai_gpt_image_2"); fill();
  expect(screen.getByLabelText("参考图片 ID（每行一个）")).toHaveValue("a1");
  await click("准备任务");
  expect(f.request.mock.calls[1][2]?.body).toMatchObject({ request: { options: { model: "openai_gpt_image_2" },
    reuse: { taskId: sourceId, referenceAssetIds: ["a1"] } } });
  f.view.rerender(<LumaMcpCreationPanel {...f.props} credentialId="other" />);
  expect(screen.getByLabelText("使用已保存任务的素材")).not.toBeChecked();
  expect(screen.queryByLabelText("素材来源任务 ID")).not.toBeInTheDocument();
});

it("reframes a saved video by asset ID without inventing MIME or fetching its URL", async () => {
  const f = await fixture("video"); await click("扩画此视频"); fill();
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  expect(screen.queryByLabelText("源视频类型")).not.toBeInTheDocument();
  change("画面比例", "16:9"); await click("准备任务");
  expect(f.request.mock.calls[1][2]?.body).toMatchObject({ request: { kind: "video",
    options: { model: "luma_ray3_2", operation: "reframe", aspectRatio: "16:9" },
    reuse: { taskId: sourceId, sourceAssetId: "a1" } } });
});

it("rejects mixed sources, duplicate handles and combined reference overflow", () => {
  const draft = { ...newMcpDraft("luma_uni_1", "edit"), reuseEnabled: true, reuseTask: sourceId, sourceAsset: "a1" };
  expect(mcpDraftOptions(draft).valid).toBe(true);
  expect(mcpDraftOptions({ ...draft, source: "https://cdn.example.com/a" }).valid).toBe(false);
  expect(mcpDraftOptions({ ...draft, referenceAssets: "artifact:a1" }).valid).toBe(false);
  expect(mcpDraftOptions({ ...draft, referenceAssets: "a2", references: Array(8).fill("https://cdn.example.com/a").join("\n") }).valid).toBe(false);
});

it("keeps the draft when secret access is granted without submitting it automatically", async () => {
  const f = await fixture();
  f.view.rerender(<LumaMcpCreationPanel {...f.props} secretGrant={null} />);
  change("创作操作", "edit");
  fireEvent.click(screen.getByLabelText("使用已保存任务的素材"));
  change("素材来源任务 ID", sourceId); change("源素材 ID", "a1"); fill();
  await click("准备任务");
  expect(f.props.onRequestSecretAccess).toHaveBeenCalledOnce();
  expect(f.request).toHaveBeenCalledOnce();
  f.view.rerender(<LumaMcpCreationPanel {...f.props} />);
  expect(screen.getByLabelText("素材来源任务 ID")).toHaveValue(sourceId);
  expect(screen.getByLabelText("源素材 ID")).toHaveValue("a1");
  expect(f.request).toHaveBeenCalledOnce();
  await click("准备任务");
  expect(f.request).toHaveBeenCalledTimes(2);
});
