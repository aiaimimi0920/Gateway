// Normal main UI → management TCP → test-only Luma remap. Requires the opt-in Rust host.
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
const host = path.resolve(values.host);
const output = path.resolve(values.output);
const ready = JSON.parse(await readFile(path.join(host, "ready.json"), "utf8"));
assert.equal(ready.syntheticOnly, true);
assert.equal(ready.scenario, "luma-files-normal-main-v1");
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output);
const require = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
const { chromium } = require("playwright"); // Existing locked dependency; no install or browser download.
const calls = [], responses = [], errors = [], external = [], checkpoints = [], sessionResponses = [];
let server, browser, page, success = false, proxyClosed = false;
const record = (name, details = {}) => checkpoints.push({ name, fileRequests: calls.length, ...details });
const button = name => page.getByRole("button", { name, exact: true });
const write = (name, value) => writeFile(path.join(output, name), JSON.stringify(value, null, 2) + "\n");
try {
  server = await serveMain(values.ui, ready.origin);
  browser = await chromium.launch({ channel: values.channel, headless: true });
  const context = await browser.newContext({ locale: "zh-CN", timezoneId: "UTC", viewport: { width: 1440, height: 1100 },
    permissions: ["clipboard-read", "clipboard-write"], serviceWorkers: "block" });
  // Only this newly created fixture origin is reachable from the disposable browser context.
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === server.origin) return route.continue();
    external.push(new URL(route.request().url()).origin);
    return route.abort();
  });
  page = await context.newPage();
  page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => {
    if (!request.url().endsWith("/console/luma-agents-files")) return;
    const type = request.headers()["content-type"];
    const body = type.startsWith("application/json") ? request.postDataJSON() : null;
    calls.push({ method: request.method(), contentType: type, action: body?.operation.action ?? "upload",
      providerId: body?.providerId ?? "multipart", credentialId: body?.credentialId ?? "multipart" });
  });
  page.on("response", response => {
    if (response.url().endsWith("/console/luma-agents-files")) responses.push(response.status());
    if (response.url().includes("/console/session/")) sessionResponses.push({
      path: new URL(response.url()).pathname, status: response.status() });
  });
  await page.goto(server.origin + "/ui/");
  await page.locator("#login-token").fill("console-management-contract-token");
  await button("登录").click();
  await page.getByRole("button", { name: "凭据池", exact: true }).click();
  await button("exact").click();
  await button("编辑账号").click();
  const files = page.getByRole("region", { name: "Luma 文件", exact: true });
  await files.waitFor();
  await writeFile(path.join(output, "initial.ax.txt"), await page.locator("body").ariaSnapshot());
  // Login obtains a short-lived grant through the normal session owner; no storage injection.
  const upload = files.locator('input[type="file"]');
  await upload.setInputFiles({ name: "参考.png", mimeType: "image/png", buffer: Buffer.from([1, 2, 3]) });
  await files.getByLabel("文件用途", { exact: true }).selectOption("reference");
  await button("上传文件").click();
  await files.getByRole("status").filter({ hasText: "处理中" }).waitFor();
  assert.equal(await button("复制 file_id").isDisabled(), true);
  assert.equal(calls.length, 1);
  record("inline-pending");
  await button("刷新文件状态").click();
  await files.getByRole("status").filter({ hasText: "可用" }).waitFor();
  await button("复制 file_id").click();
  await button("已复制").waitFor();
  assert.equal(await page.evaluate(() => navigator.clipboard.readText()), "d290f1ee-6c54-4b01-90e6-d701748f0851");
  record("inline-ready-copied");
  const closeAccount = () => page.getByRole("dialog", { name: "编辑账号", exact: true })
    .getByRole("button", { name: "取消", exact: true }).click();
  await closeAccount();
  await page.getByRole("combobox", { name: "切换账号", exact: true }).selectOption("luma-pipeline:unrelated");
  await button("编辑账号").click();
  await files.waitFor();
  assert.equal(await files.getByLabel("file_id", { exact: true }).count(), 0);
  assert.equal(await upload.inputValue(), "");
  await closeAccount();
  await page.getByRole("combobox", { name: "切换账号", exact: true }).selectOption("luma-pipeline:exact");
  await button("编辑账号").click();
  await files.waitFor();
  assert.equal(await files.getByLabel("file_id", { exact: true }).count(), 0);
  assert.equal(calls.length, 2);
  record("account-switch-clears-results");
  await button("读取文件列表").click();
  await button("下一页").click();
  await button("下一页").waitFor({ state: "hidden" });
  record("pagination");
  await upload.setInputFiles({ name: "large.png", mimeType: "image/png", buffer: Buffer.alloc(8 * 1024 * 1024 + 1, 7) });
  await files.getByLabel("文件用途", { exact: true }).selectOption("reference");
  await files.getByLabel("过期时间（可选）", { exact: true }).fill("2099-01-01T00:00");
  await files.getByLabel("用户标记（可选）", { exact: true }).fill("owner-tag");
  await button("上传文件").click();
  await files.getByRole("status").filter({ hasText: "处理中" }).waitFor();
  assert.equal(await files.getByLabel("file_id", { exact: true }).inputValue(), "d290f1ee-6c54-4b01-90e6-d701748f0852");
  await files.getByText("用户标记 · owner-tag", { exact: true }).waitFor();
  assert.match(calls.at(-1).contentType, /^multipart\/form-data; boundary=/);
  record("presigned-pending");
  await button("刷新文件状态").click();
  await files.getByRole("status").filter({ hasText: "可用" }).waitFor();
  await page.screenshot({ path: path.join(output, "ready.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await button("删除文件").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "ready-narrow.png") });
  const count = calls.length;
  await button("删除文件").click();
  await button("取消删除").click();
  assert.equal(calls.length, count);
  record("delete-cancelled");
  await button("删除文件").click();
  await button("确认删除文件").click();
  await files.getByText("没有文件", { exact: true }).waitFor();
  record("delete-confirmed");
  assert.equal(calls.length, 7);
  assert.deepEqual(responses, Array(7).fill(200));
  assert.equal(calls.filter(call => call.action === "upload").length, 2);
  assert.ok(calls.filter(call => call.credentialId !== "multipart").every(call =>
    call.providerId === "luma-pipeline" && call.credentialId === "exact"));
  assert.deepEqual(errors, []);
  assert.deepEqual(external, []);
  assert.ok(sessionResponses.some(response => response.path.endsWith("/confirm-secret-access") && response.status === 200));
  await writeFile(path.join(output, "final.ax.txt"), await page.locator("body").ariaSnapshot());
  success = true;
} catch (error) {
  errors.push(error.stack ?? String(error));
  if (page && !page.isClosed()) {
    await writeFile(path.join(output, "failure.ax.txt"), await page.locator("body").ariaSnapshot());
    await page.screenshot({ path: path.join(output, "failure.png") });
  }
  process.exitCode = 1;
} finally {
  try {
    await browser?.close();
    await server?.close();
    proxyClosed = true;
  } catch (error) {
    success = false;
    process.exitCode = 1;
    errors.push(error.stack ?? String(error));
  }
  finally {
    // Publish a complete command atomically so the Rust owner never reads a partial JSON write.
    await writeFile(path.join(host, "browser-command.tmp"), JSON.stringify({ action: success ? "finish" : "abort" }));
    await rename(path.join(host, "browser-command.tmp"), path.join(host, "command.json"));
    await write("browser-result.json", { status: success ? "passed" : "failed", browserClosed: !browser?.isConnected(),
      proxyClosed, calls, responses, sessionResponses, errors, external, checkpoints, scope: "normal main UI; synthetic upstream only" });
  }
}
console.log(JSON.stringify({ status: success ? "passed" : "failed", output, errors }));
