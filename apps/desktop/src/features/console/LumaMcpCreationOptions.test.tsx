import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { LumaMcpCreationPanel } from "./LumaMcpCreationPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
function fixture() {
  const request = vi.fn(async () => { throw new Error("synthetic"); });
  render(<LumaMcpCreationPanel client={{ request } as unknown as GatewayApiClient} managementToken="owner"
    secretGrant="grant" providerId="mcp" credentialId="saved" expectedRevision="r1" disabled={false}
    onRequestSecretAccess={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("提示词"), { target: { value: "Keep the subject, change the sky" } });
  fireEvent.change(screen.getByLabelText("会话预算上限（credits）"), { target: { value: "40" } });
  return request;
}
const change = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } });
const prepare = async () => { await act(async () => fireEvent.click(screen.getByRole("button", { name: "准备任务" }))); };

it("collects explicit GPT Image 2 parameters and reference images without charging", async () => {
  const request = fixture();
  change("创作模型", "openai_gpt_image_2");
  change("图片质量", "high"); change("图片分辨率", "4K"); change("图片格式", "webp");
  change("参考图片 URL（每行一个）", "https://cdn.example.com/reference.png");
  await prepare();
  expect(request).toHaveBeenCalledOnce();
  expect(request.mock.calls[0]).toEqual(expect.arrayContaining([expect.objectContaining({ body: expect.objectContaining({
    action: "prepare", request: expect.objectContaining({ kind: "image", options: expect.objectContaining({
      model: "openai_gpt_image_2", referenceUrls: ["https://cdn.example.com/reference.png"],
      parameters: { quality: "high", resolution: "4K", outputFormat: "webp" },
    }) }),
  }) })]));
});

it("requires an edit source and removes incompatible image parameters when switching to video", async () => {
  const request = fixture(); change("创作模型", "openai_gpt_image_2"); change("图片质量", "high");
  change("创作模型", "luma_ray3_2"); change("创作操作", "edit");
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  change("源素材 URL", "https://cdn.example.com/source.mp4");
  change("重绘强度", "flex_2"); await prepare();
  const body = (request.mock.calls[0] as unknown as [unknown, unknown, { body: { request: { options: unknown } } }])[2].body;
  expect(body.request.options).toEqual({ model: "luma_ray3_2", operation: "edit",
    sourceUrl: "https://cdn.example.com/source.mp4", sourceMediaType: "video/mp4", video: { edit: { strength: "flex_2" } } });
});

it("requires an output ratio for video reframe and rejects unsafe source URLs", async () => {
  const request = fixture(); change("创作模型", "luma_ray3_2"); change("创作操作", "reframe");
  change("源素材 URL", "http://cdn.example.com/source.mp4"); change("画面比例", "16:9");
  expect(screen.getByRole("button", { name: "准备任务" })).toBeDisabled();
  change("源素材 URL", "https://cdn.example.com/source.mp4"); change("视频时长", "10s");
  await prepare();
  expect(request).toHaveBeenCalledOnce();
});
