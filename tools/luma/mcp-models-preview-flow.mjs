// New model controls on the real form; uses the existing no-network synthetic client.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import path from "node:path";

export async function checkModelProfiles(page, dialog, output) {
  const field = name => dialog.getByLabel(name, { exact: true });
  const button = name => dialog.getByRole("button", { name, exact: true });
  assert.equal(await field("创作模型").locator("option").count(), 18);
  await field("提示词").fill("A blue circle");
  await field("会话预算上限（credits）").fill("50");
  await field("创作模型").selectOption("google_nano_banana_2");
  await field("图片分辨率").selectOption("512");
  await field("画面比例").selectOption("8:1");
  await field("推理程度").selectOption("high");
  await field("搜索增强").selectOption("false");
  await field("温度").fill("0");
  await field("创作模型").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "nano-model-wide.png") });
  const generate = async () => {
    await button("准备任务").click();
    assert.equal(await button("提交生成").isEnabled(), false);
    await field("确认使用此 Luma 账号的 credits 生成").check();
    await button("提交生成").click();
    await button("查询结果与费用").click();
    await dialog.getByText("会话费用：9 credits · 已结算", { exact: true }).waitFor();
  };
  await generate(); await button("另一任务").click();
  await field("创作模型").selectOption("bytedance_seedream_5_flash");
  assert.equal(await field("画面比例").count(), 0);
  await field("图片尺寸").fill("100x100");
  assert.equal(await button("准备任务").isEnabled(), false);
  await field("图片尺寸").fill("1536x1024");
  await field("图片格式").selectOption("png");
  await page.setViewportSize({ width: 390, height: 844 });
  for (const label of ["创作模型", "图片尺寸", "图片格式"]) {
    await field(label).scrollIntoViewIfNeeded();
    const box = await field(label).boundingBox();
    assert.ok(box && box.x >= 0 && box.x + box.width <= 390, label);
  }
  await page.screenshot({ path: path.join(output, "seedream-model-narrow.png") });
  await writeFile(path.join(output, "seedream.ax.txt"), await dialog.ariaSnapshot());
  await generate(); await button("另一任务").click();
  await field("创作模型").selectOption("luma_ray3_14_hdr");
  assert.deepEqual(await field("视频时长").locator("option").allTextContents(), ["默认", "5s"]);
  assert.deepEqual(await field("视频分辨率").locator("option").allTextContents(), ["默认", "720p", "1080p"]);
  assert.equal(await field("创作操作").count(), 0);
  await field("视频分辨率").selectOption("1080p");
  await field("创作模型").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "hdr-model-narrow.png") });
  await generate();
  const calls = await page.evaluate(() => window.syntheticRequests);
  assert.deepEqual(calls.map(c => c.body.action), Array(3).fill(["prepare", "start", "read"]).flat());
  for (const index of [0, 3, 6]) assert.deepEqual(calls[index].body.request, calls[index + 1].body.request);
  assert.deepEqual(calls[0].body.request.options.parameters, { resolution: "512", thinkingLevel: "high", enableSearch: false, temperature: 0 });
  assert.deepEqual(calls[3].body.request.options.parameters, { size: "1536x1024", outputFormat: "png" });
  assert.equal(calls[6].body.request.options.model, "luma_ray3_14_hdr");
  assert.equal(calls[6].body.request.kind, "video");
  return calls;
}
