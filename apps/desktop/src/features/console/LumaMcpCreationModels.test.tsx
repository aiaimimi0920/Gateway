import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { LumaMcpCreationPanel } from "./LumaMcpCreationPanel";
import { mcpDraftOptions, newMcpDraft } from "./lumaMcpCreationDraft";
import { mcpModels, type McpModel } from "./lumaMcpCreationModels";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const change = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } });
function fixture() {
  const request = vi.fn(async () => { throw new Error("synthetic"); });
  render(<LumaMcpCreationPanel client={{ request } as unknown as GatewayApiClient} managementToken="owner"
    secretGrant="grant" providerId="mcp" credentialId="saved" expectedRevision="r1" disabled={false}
    onRequestSecretAccess={vi.fn()} />);
  change("提示词", "A blue circle"); change("会话预算上限（credits）", "50");
  return request;
}
it.each(Object.keys(mcpModels) as McpModel[])("preserves explicitly selected model %s", model => {
  const result = mcpDraftOptions(newMcpDraft(model));
  expect(result.valid).toBe(true);
  expect(result.kind).toBe(mcpModels[model].kind);
  if (["luma_uni_1", "luma_ray3_2"].includes(model)) expect(result.options).toBeUndefined();
  else expect(result.options?.model).toBe(model);
});
it("exposes only supported Nano Banana options and sends explicit false and zero", async () => {
  const request = fixture(); change("创作模型", "google_nano_banana_2");
  expect(screen.queryByLabelText("图片背景")).not.toBeInTheDocument();
  change("图片分辨率", "512"); change("画面比例", "8:1");
  change("搜索增强", "false"); change("温度", "0"); change("推理程度", "high");
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "准备任务" })));
  expect(request).toHaveBeenCalledOnce();
  expect(request.mock.calls[0]).toEqual(expect.arrayContaining([expect.objectContaining({ body: expect.objectContaining({
    action: "prepare", request: expect.objectContaining({ options: {
      model: "google_nano_banana_2", operation: "generate", aspectRatio: "8:1",
      parameters: { resolution: "512", enableSearch: false, temperature: 0, thinkingLevel: "high" },
    } }),
  }) })]));
});
it("validates Flash dimensions and clears its fields when switching to a different model", () => {
  fixture(); change("创作模型", "bytedance_seedream_5_flash");
  expect(screen.queryByLabelText("画面比例")).not.toBeInTheDocument();
  change("图片尺寸", "100x100"); expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  change("图片尺寸", "1536x1024"); expect(screen.getByRole("button", { name: "准备任务" })).toBeEnabled();
  change("创作模型", "google_nano_banana_2_lite");
  expect(screen.queryByLabelText("图片尺寸")).not.toBeInTheDocument();
  expect(screen.getByLabelText("图片分辨率").textContent).toBe("默认1K");
});
it("requires compatible Ideogram quality, size and ordered reference inputs", () => {
  fixture(); change("创作模型", "ideogram_4_6"); change("图片质量", "very_low"); change("图片尺寸", "4096x4096");
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  change("图片尺寸", "2048x2048"); change("提示词增强", "on");
  change("参考图片 URL（每行一个）", "https://example.com/ref");
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  change("提示词增强", ""); expect(screen.getByRole("button", { name: "准备任务" })).toBeEnabled();
});
it("keeps HDR and legacy Ray frame combinations explicit", () => {
  fixture(); change("创作模型", "luma_ray3_14_hdr");
  expect(screen.queryByLabelText("创作操作")).not.toBeInTheDocument();
  expect(screen.getByLabelText("视频时长").textContent).toBe("默认5s");
  expect(screen.getByLabelText("视频分辨率").textContent).toBe("默认720p1080p");
  change("创作模型", "luma_ray3_14"); change("视频时长", "10s");
  change("起始帧 URL", "https://example.com/start");
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  change("视频时长", "5s"); expect(screen.getByRole("button", { name: "准备任务" })).toBeEnabled();
  expect(screen.queryByLabelText("关键帧（每行：帧序号 URL）")).not.toBeInTheDocument();
});
it("counts URL and saved-asset references together using each model limit", () => {
  const result = mcpDraftOptions({ ...newMcpDraft("openai_gpt_image_1_5"), reuseEnabled: true,
    reuseTask: "luma-mcp-11111111-1111-4111-8111-111111111111", referenceAssets: "a1\na2\na3\na4", references: "https://example.com/ref" });
  expect(result.valid).toBe(false);
});
