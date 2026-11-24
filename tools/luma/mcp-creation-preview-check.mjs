// Actual form and layout, synthetic client only; all non-loopback requests are blocked.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { parseArgs } from "node:util";
import { checkModelProfiles } from "./mcp-models-preview-flow.mjs";
import { checkReview } from "./mcp-review-preview-flow.mjs";

const { values } = parseArgs({ options: { origin: { type: "string" }, output: { type: "string" }, reuse: { type: "boolean" }, models: { type: "boolean" }, review: { type: "boolean" } } });
if ([values.reuse, values.models, values.review].filter(Boolean).length > 1) throw new Error("Select one of --reuse, --models or --review.");
if (!values.origin || !values.output) throw new Error("Use --origin http://127.0.0.1:<port> --output <new-directory>");
const origin = new URL(values.origin);
assert.equal(origin.protocol, "http:"); assert.equal(origin.hostname, "127.0.0.1"); assert.ok(origin.port);
const output = path.resolve(values.output);
await mkdir(path.dirname(output), { recursive: true }); await mkdir(output);
const require = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
const { chromium } = require("playwright");
const browser = await chromium.launch({ channel: "msedge", headless: true });
const errors = [], external = [];
let status = "failed", calls = [], page;
try {
  const context = await browser.newContext({ locale: "zh-CN", viewport: { width: 1440, height: 1000 }, serviceWorkers: "block" });
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === origin.origin) return route.continue();
    if (values.review && route.request().url() === "https://cdn.example.com/review.png") return route.fulfill({
      contentType: "image/svg+xml", body: '<svg xmlns="http://www.w3.org/2000/svg" width="320" height="180"><circle cx="160" cy="90" r="65" fill="#4080dd"/></svg>' });
    external.push(route.request().url()); return route.abort();
  });
  page = await context.newPage(); page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(origin.origin + (values.review ? "/?mcpReview=1" : ""));
  await page.getByRole("button", { name: "关闭测试弹窗", exact: true }).click();
  await page.getByRole("button", { name: "打开 MCP 创作预览", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "合成 MCP 创作", exact: true });
  const field = name => dialog.getByLabel(name, { exact: true });
  const click = name => dialog.getByRole("button", { name, exact: true }).click();
  assert.equal(await page.evaluate(() => window.syntheticRequests.length), 0);
  if (values.review) {
    calls = await checkReview(page, dialog, output);
  } else if (values.models) {
    calls = await checkModelProfiles(page, dialog, output);
  } else {
  await field("创作模型").selectOption("openai_gpt_image_2");
  await field("图片分辨率").selectOption("4K"); await field("图片质量").selectOption("high");
  await field("图片格式").selectOption("webp");
  await field("参考图片 URL（每行一个）").fill("https://cdn.example.com/source.png");
  await field("提示词").fill("Keep the subject and change the sky");
  await field("会话预算上限（credits）").fill("40");
  await field("创作模型").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "gpt-options.png") });
  await click("准备任务");
  assert.equal(await dialog.getByRole("button", { name: "提交生成", exact: true }).isEnabled(), false);
  assert.equal(await field("创作模型").isEnabled(), false);
  await field("确认使用此 Luma 账号的 credits 生成").check(); await click("提交生成");
  assert.equal(await dialog.getByRole("button", { name: "提交生成", exact: true }).count(), 0);
  await click("查询结果与费用");
  await dialog.getByText("会话费用：9 credits · 已结算", { exact: true }).waitFor();
  assert.equal(await dialog.getByRole("link", { name: "打开生成结果", exact: true }).getAttribute("href"), "https://cdn.example.com/result");
  await page.screenshot({ path: path.join(output, "gpt-result.png") });
  if (values.reuse) {
    const sourceId = await field("任务 ID").inputValue();
    await click("用作参考图片");
    await field("创作模型").selectOption("openai_gpt_image_2");
    assert.equal(await field("素材来源任务 ID").inputValue(), sourceId);
    assert.equal(await field("参考图片 ID（每行一个）").inputValue(), "synthetic-asset");
    assert.equal(await field("会话预算上限（credits）").inputValue(), "");
    assert.equal(await page.evaluate(() => window.syntheticRequests.length), 3);
    await field("提示词").fill("Use the saved image as reference");
    await field("会话预算上限（credits）").fill("13");
    await field("素材来源任务 ID").scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.join(output, "reuse-reference.png") });
    await click("准备任务");
    assert.equal(await dialog.getByRole("button", { name: "提交生成", exact: true }).isEnabled(), false);
    await field("确认使用此 Luma 账号的 credits 生成").check(); await click("提交生成");
    await click("查询结果与费用");
    const childId = await field("任务 ID").inputValue();
    assert.notEqual(childId, sourceId);
    await click("编辑此素材");
    assert.equal(await field("素材来源任务 ID").inputValue(), childId);
    assert.equal(await field("源素材 ID").inputValue(), "synthetic-asset");
    assert.equal(await field("源素材 URL").count(), 0);
    await field("提示词").fill("Edit the saved image");
    await field("会话预算上限（credits）").fill("12");
    await page.setViewportSize({ width: 390, height: 844 });
    for (const label of ["素材来源任务 ID", "源素材 ID", "提示词"]) {
      const input = field(label); await input.scrollIntoViewIfNeeded();
      const bounds = await input.boundingBox(); assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390, label);
    }
    await page.screenshot({ path: path.join(output, "reuse-edit-narrow.png") });
    await writeFile(path.join(output, "reuse.ax.txt"), await dialog.ariaSnapshot());
    await click("准备任务"); await field("确认使用此 Luma 账号的 credits 生成").check(); await click("提交生成");
    calls = await page.evaluate(() => window.syntheticRequests);
    assert.deepEqual(calls.map(call => call.body.action), ["prepare", "start", "read", "prepare", "start", "read", "prepare", "start"]);
    assert.deepEqual(calls[3].body.request, calls[4].body.request);
    assert.deepEqual(calls[6].body.request, calls[7].body.request);
    assert.deepEqual(calls[3].body.request.reuse, { taskId: sourceId, referenceAssetIds: ["synthetic-asset"] });
    assert.deepEqual(calls[6].body.request.reuse, { taskId: childId, sourceAssetId: "synthetic-asset", referenceAssetIds: [] });
    assert.equal(calls[6].body.request.options.sourceUrl, undefined);
  } else {
  await click("另一任务");
  await field("创作模型").selectOption("luma_ray3_2"); await field("创作操作").selectOption("reframe");
  assert.equal(await dialog.getByRole("button", { name: "准备任务", exact: true }).isEnabled(), false);
  await field("源素材 URL").fill("https://cdn.example.com/source.webm");
  await field("源视频类型").selectOption("video/webm"); await field("画面比例").selectOption("16:9");
  await field("视频时长").selectOption("10s"); await field("视频分辨率").selectOption("720p");
  await field("源画面位置（x, y, 宽, 高）").fill("-0.5, 0.5, 1.5, 0.5");
  await page.setViewportSize({ width: 390, height: 844 });
  for (const label of ["创作模型", "源素材 URL", "源画面位置（x, y, 宽, 高）"]) {
    const input = field(label); await input.scrollIntoViewIfNeeded();
    const bounds = await input.boundingBox(); assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390, label);
  }
  await page.screenshot({ path: path.join(output, "video-narrow.png") });
  await writeFile(path.join(output, "video.ax.txt"), await dialog.ariaSnapshot());
  await click("准备任务"); await field("确认使用此 Luma 账号的 credits 生成").check(); await click("提交生成");
  await click("取消生成"); await click("结束会话");
  calls = await page.evaluate(() => window.syntheticRequests);
  assert.deepEqual(calls.map(call => call.body.action), ["prepare", "start", "read", "prepare", "start", "cancel", "end"]);
  assert.deepEqual(calls[0].body.request, calls[1].body.request);
  assert.deepEqual(calls[3].body.request, calls[4].body.request);
  assert.equal(calls[0].body.request.options.parameters.quality, "high");
  assert.equal(calls[3].body.request.options.sourceMediaType, "video/webm");
  assert.equal(calls[3].body.request.options.parameters, undefined);
  }
  }
  assert.deepEqual(errors, []); assert.deepEqual(external, []);
  status = "passed";
} catch (error) {
  errors.push(error.stack ?? String(error)); process.exitCode = 1;
  if (page) await writeFile(path.join(output, "failure.ax.txt"), await page.locator("body").ariaSnapshot());
} finally {
  await browser.close();
  await writeFile(path.join(output, "result.json"), JSON.stringify({ status, browserClosed: !browser.isConnected(), calls, errors, external,
    scope: "synthetic component preview only; Rust management transport tested separately" }, null, 2) + "\n");
}
console.log(JSON.stringify({ status, errors, output }));
