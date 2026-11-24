// Verify the saved-account MCP catalog through the normal UI and a non-forwarding Rust peer.
import assert from "node:assert/strict";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { parseArgs } from "node:util";
import { serveMain } from "./normal-main-server.mjs";

const { values } = parseArgs({ options: { host: { type: "string" }, ui: { type: "string" },
  output: { type: "string" }, channel: { type: "string", default: "msedge" } } });
if (!values.host || !values.ui || !values.output) {
  throw new Error("Use --host <fixture-root> --ui <normal-main-build> --output <new-directory> [--channel msedge]");
}
const host = path.resolve(values.host), output = path.resolve(values.output);
const ready = JSON.parse(await readFile(path.join(host, "ready.json"), "utf8"));
assert.equal(ready.syntheticOnly, true);
assert.equal(ready.scenario, "luma-mcp-normal-main-v1");
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output);
const require = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
const { chromium } = require("playwright");
const calls = [], responses = [], errors = [], external = [], checkpoints = [], sessionResponses = [];
let server, browser, page, success = false, proxyClosed = false;
const catalogPath = url => /^\/v1\/internal\/gateway\/console\/luma-mcp-(tools|models)$/.test(new URL(url).pathname);
const button = name => page.getByRole("button", { name, exact: true });
const record = name => checkpoints.push({ name, catalogRequests: calls.length });
async function openAccount() {
  await button("编辑账号").click();
  await button("读取模型").waitFor();
}
async function closeAccount() {
  await page.getByRole("dialog", { name: "Luma MCP · 重新授权", exact: true }).getByRole("button", { name: "关闭", exact: true }).last().click();
}
async function read(name, status = 200) {
  const pending = page.waitForResponse(response => catalogPath(response.url()));
  await button(name).click();
  const response = await pending;
  assert.equal(response.status(), status);
  assert.equal(response.headers()["cache-control"], "no-store");
  const text = await response.text();
  for (const secret of ["synthetic-access", "synthetic-other", "synthetic-private-upstream-error"]) assert.ok(!text.includes(secret));
  responses.push({ path: new URL(response.url()).pathname, status: response.status() });
}
try {
  server = await serveMain(values.ui, ready.origin);
  browser = await chromium.launch({ channel: values.channel, headless: true });
  const context = await browser.newContext({ locale: "zh-CN", timezoneId: "UTC", viewport: { width: 1440, height: 1100 }, serviceWorkers: "block" });
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === server.origin) return route.continue();
    external.push(new URL(route.request().url()).origin); return route.abort();
  });
  page = await context.newPage(); page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => {
    if (catalogPath(request.url())) calls.push({ path: new URL(request.url()).pathname, body: request.postDataJSON() });
  });
  page.on("response", response => {
    if (response.url().includes("/console/session/")) sessionResponses.push({ path: new URL(response.url()).pathname, status: response.status() });
  });
  await page.goto(server.origin + "/ui/");
  await page.locator("#login-token").fill("console-management-contract-token");
  await button("登录").click(); await button("凭据池").click();
  await button(ready.credential).click(); await openAccount();
  assert.equal(calls.length, 0); record("opening-does-not-read");
  assert.equal(await button("导出工具目录").isDisabled(), true);
  await writeFile(path.join(output, "initial.ax.txt"), await page.locator("body").ariaSnapshot());
  await read("读取工具目录");
  assert.equal(await page.getByLabel("工具", { exact: true }).inputValue(), "luma_model_list");
  assert.equal(await page.getByLabel("说明", { exact: true }).inputValue(), "<script>window.catalogInjected=true</script>");
  assert.equal(await page.evaluate(() => Boolean(window.catalogInjected)), false);
  assert.equal(await button("执行工具").count(), 0);
  record("tool-catalog-display-only");
  const downloadPending = page.waitForEvent("download");
  await button("导出工具目录").click();
  const download = await downloadPending;
  assert.match(download.suggestedFilename(), /^luma-mcp-catalog-.*\.json$/);
  const snapshotPath = path.join(output, "catalog-snapshot.json");
  await download.saveAs(snapshotPath);
  const snapshotText = await readFile(snapshotPath, "utf8"), snapshot = JSON.parse(snapshotText);
  assert.equal(snapshot.format, "gateway-luma-mcp-catalog");
  assert.equal(snapshot.resource, "https://agents.lumalabs.ai/mcp");
  assert.equal(snapshot.protocolVersion, "2025-03-26");
  assert.ok(Number.isFinite(Date.parse(snapshot.capturedAt)));
  assert.equal(snapshot.tools.length, 1);
  assert.equal(snapshot.tools[0].title, "合成模型列表");
  assert.deepEqual(snapshot.tools[0].inputSchema, { type: "object", properties: {} });
  assert.deepEqual(snapshot.tools[0].outputSchema, { type: "object", properties: { models: { type: "array" } } });
  assert.deepEqual(snapshot.tools[0].annotations, { readOnlyHint: true, idempotentHint: true });
  for (const secret of ["synthetic-access", "console-management-contract-token", "excluded-tool-metadata", ready.credential, ready.revision]) assert.ok(!snapshotText.includes(secret));
  assert.equal(calls.length, 1); record("catalog-download-without-upstream-call");
  await button("导出工具目录").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "catalog-export.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await button("导出工具目录").scrollIntoViewIfNeeded();
  const exportBounds = await button("导出工具目录").boundingBox();
  assert.ok(exportBounds && exportBounds.x >= 0 && exportBounds.x + exportBounds.width <= 390 && exportBounds.y >= 0 && exportBounds.y + exportBounds.height <= 844);
  await page.screenshot({ path: path.join(output, "catalog-export-narrow.png") });
  await page.setViewportSize({ width: 1440, height: 1100 });
  await read("读取模型");
  assert.equal(await page.getByLabel("可选模型", { exact: true }).inputValue(), "synthetic-image");
  assert.equal(await page.getByLabel("公布费率", { exact: true }).inputValue(), "未知");
  await read("读取模型参数");
  assert.match(await page.getByLabel("模型参数", { exact: true }).inputValue(), /standard/);
  await page.getByLabel("可选模型", { exact: true }).selectOption("synthetic-audio");
  assert.equal(await button("读取模型参数").isDisabled(), true);
  assert.equal(await page.getByLabel("模型参数", { exact: true }).count(), 0);
  assert.equal(calls.length, 3);
  await page.getByLabel("可选模型", { exact: true }).selectOption("synthetic-video");
  await read("读取模型参数");
  const options = page.getByLabel("模型参数", { exact: true });
  await options.scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "models.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await options.scrollIntoViewIfNeeded();
  const bounds = await options.boundingBox();
  assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390 && bounds.y >= 0 && bounds.y + bounds.height <= 844);
  await page.screenshot({ path: path.join(output, "models-narrow.png") });
  record("image-video-options-and-audio-boundary");
  await closeAccount();
  await page.getByRole("combobox", { name: "切换账号", exact: true }).selectOption("luma-mcp:other-mcp");
  await openAccount();
  assert.equal(await page.getByLabel("可选模型", { exact: true }).count(), 0);
  assert.equal(await page.getByLabel("工具", { exact: true }).count(), 0);
  assert.equal(await button("导出工具目录").isDisabled(), true);
  await closeAccount();
  await page.getByRole("combobox", { name: "切换账号", exact: true }).selectOption("luma-mcp:saved-mcp");
  await openAccount();
  assert.equal(calls.length, 4); record("account-switch-clears-results");
  await read("读取模型");
  await page.getByText("模型列表为空", { exact: true }).waitFor();
  assert.equal(await page.getByLabel("可选模型", { exact: true }).count(), 0);
  await read("读取工具目录", 503);
  await page.getByText("工具目录读取失败，请检查授权与配置版本后重试。", { exact: true }).waitFor();
  assert.equal(await button("导出工具目录").isDisabled(), true);
  await writeFile(path.join(output, "final.ax.txt"), await page.locator("body").ariaSnapshot());
  await page.screenshot({ path: path.join(output, "empty-error.png") });
  await closeAccount();
  assert.equal(calls.length, 6);
  for (const { body } of calls) {
    assert.equal(body.providerId, ready.provider); assert.equal(body.credentialId, ready.credential);
    assert.equal(body.expectedRevision, ready.revision);
  }
  assert.deepEqual(calls.filter(call => call.body.action === "describe").map(call => [call.body.model, call.body.type]), [["synthetic-image", "image"], ["synthetic-video", "video"]]);
  assert.ok(sessionResponses.some(item => item.path.endsWith("/confirm-secret-access") && item.status === 200));
  assert.deepEqual(errors, []); assert.deepEqual(external, []);
  record("empty-list-and-classified-error"); success = true;
} catch (error) {
  errors.push(error.stack ?? String(error)); process.exitCode = 1;
  if (page && !page.isClosed()) {
    await writeFile(path.join(output, "failure.ax.txt"), await page.locator("body").ariaSnapshot());
    await page.screenshot({ path: path.join(output, "failure.png") });
  }
} finally {
  try { await browser?.close(); await server?.close(); proxyClosed = true; }
  catch (error) { success = false; process.exitCode = 1; errors.push(error.stack ?? String(error)); }
  finally {
    await writeFile(path.join(host, "browser-command.tmp"), JSON.stringify({ action: success ? "finish" : "abort" }));
    await rename(path.join(host, "browser-command.tmp"), path.join(host, "command.json"));
    await writeFile(path.join(output, "browser-result.json"), JSON.stringify({ status: success ? "passed" : "failed",
      browserClosed: !browser?.isConnected(), proxyClosed, calls, responses, sessionResponses, errors, external, checkpoints,
      scope: "normal main UI to management TCP and synthetic read-only MCP" }, null, 2) + "\n");
  }
}
console.log(JSON.stringify({ status: success ? "passed" : "failed", output, errors }));
