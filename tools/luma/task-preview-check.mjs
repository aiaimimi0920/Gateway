// Real components, synthetic responses; no management-server or vendor acceptance claim.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { parseArgs } from "node:util";

const { values } = parseArgs({ options: { origin: { type: "string" }, output: { type: "string" } } });
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
    external.push(route.request().url()); return route.abort();
  });
  page = await context.newPage(); page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  await page.goto(origin.origin);
  await page.getByRole("button", { name: "关闭测试弹窗", exact: true }).click();
  await page.getByRole("button", { name: "请求详情: cancelled-luma-task", exact: true }).click();
  await page.getByLabel("Luma 任务 ID", { exact: true }).waitFor();
  await page.getByText("本地媒体验证", { exact: true }).scrollIntoViewIfNeeded();
  await page.getByText("未知", { exact: true }).waitFor({ state: "visible" });
  await page.screenshot({ path: path.join(output, "audit.png") });
  await page.getByRole("button", { name: "打开任务查询", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "合成任务查询", exact: true });
  assert.equal(await page.evaluate(() => window.syntheticRequests.length), 0);
  await dialog.getByLabel("待查询任务 ID", { exact: true }).fill("d290f1ee-6c54-4b01-90e6-d701748f0851");
  for (const value of ["未知", "0", "12.5"]) {
    await dialog.getByRole("button", { name: "查询任务与费用", exact: true }).click();
    await dialog.getByText(value, { exact: true }).waitFor();
  }
  await page.screenshot({ path: path.join(output, "lookup.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  const field = dialog.getByLabel("Luma 任务 ID", { exact: true });
  await field.scrollIntoViewIfNeeded();
  const bounds = await field.boundingBox(); assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390);
  await page.screenshot({ path: path.join(output, "lookup-narrow.png") });
  await writeFile(path.join(output, "lookup.ax.txt"), await dialog.ariaSnapshot());
  await dialog.getByRole("button", { name: "关闭", exact: true }).last().click();
  await page.getByRole("button", { name: "打开任务查询", exact: true }).click();
  assert.equal(await dialog.getByLabel("待查询任务 ID", { exact: true }).inputValue(), "");
  assert.equal(await dialog.getByLabel("Luma 任务 ID", { exact: true }).count(), 0);
  calls = await page.evaluate(() => window.syntheticRequests);
  assert.equal(calls.length, 3); assert.deepEqual(errors, []); assert.deepEqual(external, []);
  status = "passed";
} catch (error) {
  errors.push(error.stack ?? String(error)); process.exitCode = 1;
  if (page) { await writeFile(path.join(output, "failure.ax.txt"), await page.locator("body").ariaSnapshot()); }
} finally {
  await browser.close();
  await writeFile(path.join(output, "result.json"), JSON.stringify({ status, browserClosed: !browser.isConnected(), calls, errors, external,
    scope: "synthetic component preview only; Rust management transport tested separately" }, null, 2) + "\n");
}
console.log(JSON.stringify({ status, errors, output }));
