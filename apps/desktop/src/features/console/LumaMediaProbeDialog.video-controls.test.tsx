import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { ComponentProps } from "react";
import { LumaMediaProbeDialog } from "./LumaMediaProbeDialog";

function props(): ComponentProps<typeof LumaMediaProbeDialog> {
  return { document: { providers: [{ id: "luma", preset: "lumalabs-api", base_url: "https://agents.lumalabs.ai/v1",
    supported_models: ["uni-1", "ray-3.2"], credentials: [{ id: "exact", api_key: "luma-api-synthetic", supported_models: ["uni-1", "ray-3.2"] }] }], model_routes: [], aliases: {} },
    providerId: "luma", credentialId: "exact", t: zh => zh, onProbe: vi.fn().mockResolvedValue(undefined), onClose: vi.fn(), locked: false, draftReady: true, error: null };
}
async function prepare(operation: string) {
  const user = userEvent.setup(), p = props();
  const view = render(<LumaMediaProbeDialog {...p} />, { wrapper: NeuroProvider });
  await user.selectOptions(screen.getByRole("combobox", { name: "操作" }), operation);
  await user.type(screen.getByRole("textbox", { name: "提示词" }), "合成视频");
  await user.type(screen.getByRole("textbox", { name: "源视频引用" }), "https://media.example.test/source.mp4");
  return { user, p, view };
}
async function confirm(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  await user.click(screen.getByRole("button", { name: "确认生成" }));
}
it("submits a bounded reframe position only after explicit account charge confirmation", async () => {
  const { user, p } = await prepare("video_reframe");
  await user.click(screen.getByRole("checkbox", { name: "自定义扩画位置" }));
  fireEvent.change(screen.getByRole("spinbutton", { name: "水平位置" }), { target: { value: "-0.5" } });
  fireEvent.change(screen.getByRole("spinbutton", { name: "相对宽度" }), { target: { value: "1.5" } });
  expect(p.onProbe).not.toHaveBeenCalled();
  await confirm(user);
  expect(p.onProbe).toHaveBeenCalledWith(expect.objectContaining({ credentialIds: ["exact"], media: {
    operation: "video_reframe", confirmCharge: true, parameters: { source: { url: "https://media.example.test/source.mp4", media_type: "video/mp4" },
      aspect_ratio: "16:9", video: { duration: "5s", resolution: "360p", source_position: { x_norm: -0.5, y_norm: 0, w_norm: 1.5, h_norm: 1 } } } } }), "luma");
});
it("blocks invalid reframe positions instead of silently dropping them", async () => {
  const { user, p } = await prepare("video_reframe");
  await user.click(screen.getByRole("checkbox", { name: "自定义扩画位置" }));
  for (const value of ["", "0", "2.01", "-1"]) {
    fireEvent.change(screen.getByRole("spinbutton", { name: "相对宽度" }), { target: { value } });
    expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  }
  await user.click(screen.getByRole("checkbox", { name: "自定义扩画位置" }));
  expect(screen.getByRole("button", { name: "运行测试" })).toBeEnabled();
  expect(p.onProbe).not.toHaveBeenCalled();
});
it("combines supported HDR, a single guide and manual signal controls without auto conditioning", async () => {
  const { user, p } = await prepare("video_edit");
  await user.click(screen.getByRole("checkbox", { name: "HDR" }));
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  await user.selectOptions(screen.getByRole("combobox", { name: "分辨率" }), "720p");
  await user.selectOptions(screen.getByRole("combobox", { name: "重绘强度" }), "flex_2");
  await user.click(screen.getByRole("checkbox", { name: "自定义约束" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "深度约束" }), "enabled");
  fireEvent.change(screen.getByRole("spinbutton", { name: "深度模糊" }), { target: { value: "0.2" } });
  await user.selectOptions(screen.getByRole("combobox", { name: "人脸约束" }), "disabled");
  await user.selectOptions(screen.getByRole("combobox", { name: "引导图模式" }), "single");
  await user.type(screen.getByRole("textbox", { name: "源图片引用" }), "https://images.example.test/guide.png");
  await confirm(user);
  expect(p.onProbe).toHaveBeenCalledWith(expect.objectContaining({ media: { operation: "video_edit", confirmCharge: true, parameters: {
    source: { url: "https://media.example.test/source.mp4", media_type: "video/mp4" }, video: { resolution: "720p", hdr: true,
      start_frame: { url: "https://images.example.test/guide.png" }, edit: { strength: "flex_2", controls: { depth: { enabled: true, blur: 0.2 }, face: { enabled: false } } } } } } }), "luma");
});
it("rejects duplicate guide frame positions and preserves paired guide ordering", async () => {
  const { user, p } = await prepare("video_edit");
  await user.selectOptions(screen.getByRole("combobox", { name: "引导图模式" }), "keyframes");
  const first = within(screen.getByRole("group", { name: "关键帧 1" }));
  await user.type(first.getByRole("textbox", { name: "源图片引用" }), "https://images.example.test/first.png");
  await user.click(screen.getByRole("button", { name: "添加关键帧" }));
  const second = within(screen.getByRole("group", { name: "关键帧 2" }));
  await user.type(second.getByRole("textbox", { name: "源图片引用" }), "https://images.example.test/second.png");
  expect(screen.getByRole("button", { name: "运行测试" })).toBeDisabled();
  fireEvent.change(second.getByRole("spinbutton", { name: "源帧序号" }), { target: { value: "24" } });
  await confirm(user);
  expect(p.onProbe).toHaveBeenCalledWith(expect.objectContaining({ media: { operation: "video_edit", confirmCharge: true, parameters: {
    source: { url: "https://media.example.test/source.mp4", media_type: "video/mp4" }, video: { resolution: "360p", edit: {
      auto_controls: true, keyframes: [{ url: "https://images.example.test/first.png" }, { url: "https://images.example.test/second.png" }], keyframe_indexes: [0, 24] } } } } }), "luma");
});
it("clears a pending charge confirmation after a prompt, saved key or draft state changes", async () => {
  const user = userEvent.setup(), p = props();
  const view = render(<LumaMediaProbeDialog {...p} />, { wrapper: NeuroProvider });
  const prompt = screen.getByRole("textbox", { name: "提示词" });
  await user.type(prompt, "原始指引");
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  fireEvent.change(prompt, { target: { value: "更改后的指引" } });
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  const document = structuredClone(p.document!);
  (document.providers[0] as { credentials: { api_key: string }[] }).credentials[0].api_key = "luma-api-replaced";
  view.rerender(<LumaMediaProbeDialog {...p} document={document} />);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "运行测试" }));
  view.rerender(<LumaMediaProbeDialog {...p} document={document} draftReady={false} />);
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(p.onProbe).not.toHaveBeenCalled();
});
