// Normal main → management TCP → synthetic Luma. No injected session, response or audit data.
import assert from "node:assert/strict";
import { mkdir, readFile, rename, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { parseArgs } from "node:util";
import { serveMain } from "./normal-main-server.mjs";

const { values } = parseArgs({ options: { host: { type: "string" }, ui: { type: "string" },
  output: { type: "string" }, channel: { type: "string", default: "msedge" } } });
if (!values.host || !values.ui || !values.output) throw new Error("Use --host <fixture-root> --ui <normal-main-build> --output <new-directory>");
const host = path.resolve(values.host), output = path.resolve(values.output);
const ready = JSON.parse(await readFile(path.join(host, "ready.json"), "utf8"));
assert.equal(ready.syntheticOnly, true); assert.equal(ready.scenario, "luma-task-normal-main-v1");
await mkdir(path.dirname(output), { recursive: true }); await mkdir(output);
const require = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
const { chromium } = require("playwright");
const calls = [], responses = [], errors = [], external = [], sessions = [], checkpoints = [];
let server, browser, page, success = false, proxyClosed = false;
const button = name => page.getByRole("button", { name, exact: true });
const record = name => checkpoints.push({ name, taskRequests: calls.length });
const facts = (scope, label) => scope.locator("dl > div").filter({ has: page.getByText(label, { exact: true }) }).locator("dd");
const expectText = async (locator, expected) => {
  await locator.filter({ hasText: new RegExp(`^${expected.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}$`) }).waitFor();
  assert.equal(await locator.textContent(), expected);
};
try {
  server = await serveMain(values.ui, ready.origin);
  browser = await chromium.launch({ channel: values.channel, headless: true });
  const context = await browser.newContext({ locale: "zh-CN", viewport: { width: 1440, height: 1100 },
    permissions: ["clipboard-read", "clipboard-write"], serviceWorkers: "block" });
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === server.origin) return route.continue();
    external.push(new URL(route.request().url()).origin); return route.abort();
  });
  page = await context.newPage(); page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => {
    if (request.url().endsWith("/console/luma-agents-task")) calls.push({ method: request.method(), body: request.postDataJSON() });
  });
  page.on("response", response => {
    if (response.url().includes("/console/session/")) sessions.push({ path: new URL(response.url()).pathname, status: response.status() });
  });
  await page.goto(server.origin + "/ui/");
  await page.locator("#login-token").fill("console-management-contract-token");
  await button("登录").click();
  await button("运维").click();
  await button(`请求详情: ${ready.auditId}`).click();
  const audit = page.getByRole("region", { name: "请求详情", exact: true });
  assert.equal(await audit.getByLabel("Luma 任务 ID", { exact: true }).inputValue(), ready.taskId);
  await expectText(facts(audit, "状态"), "已取消");
  await expectText(facts(audit, "供应商任务状态"), "排队中");
  await expectText(facts(audit, "供应商实际扣费（credits）"), "未知");
  await expectText(facts(audit, "本地媒体验证"), "未验证");
  await button("复制 Luma 任务 ID").click();
  await button("复制 Luma 任务 ID").getByRole("status").filter({ hasText: "已复制" }).waitFor();
  const taskId = await page.evaluate(() => navigator.clipboard.readText());
  assert.equal(taskId, ready.taskId);
  await facts(audit, "本地媒体验证").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "audit.png") });
  record("cancelled-audit-copied");
  await button("凭据池").click(); await button("exact").click(); await button("编辑账号").click();
  const panel = page.getByRole("region", { name: "Luma 任务查询", exact: true });
  const input = panel.getByLabel("待查询任务 ID", { exact: true });
  const read = panel.getByRole("button", { name: "查询任务与费用", exact: true });
  await panel.waitFor(); assert.equal(await read.isDisabled(), true);
  await input.fill("../foreign"); assert.equal(await read.isDisabled(), true);
  await input.fill(taskId.toUpperCase()); assert.equal(calls.length, 0);
  const query = async expected => {
    const received = page.waitForResponse(response => response.url().endsWith("/console/luma-agents-task"));
    await read.click();
    const response = await received, body = await response.json();
    responses.push({ status: response.status(), cacheControl: response.headers()["cache-control"] });
    assert.equal(response.status(), expected);
    assert.equal(response.headers()["cache-control"], "no-store");
    assert.ok(!JSON.stringify(body).includes("supplier-private-body"));
    return body;
  };
  for (const credit of [null, 0, 12.5]) {
    const body = await query(200);
    assert.equal(body.result.usage.credits_charged, credit);
    assert.deepEqual(Object.keys(body.result).sort(), ["id", "model", "state", "type", "usage"]);
    await expectText(facts(panel, "供应商实际扣费（credits）"), credit === null ? "未知" : String(credit));
    assert.equal(await panel.getByLabel("Luma 任务 ID", { exact: true }).inputValue(), taskId);
    assert.equal(await panel.getByText("本地媒体验证", { exact: true }).count(), 0);
    record(`credits-${credit}`);
  }
  await facts(panel, "供应商实际扣费（credits）").scrollIntoViewIfNeeded();
  await page.screenshot({ path: path.join(output, "lookup.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  const resultId = panel.getByLabel("Luma 任务 ID", { exact: true });
  await resultId.scrollIntoViewIfNeeded();
  const bounds = await resultId.boundingBox(); assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390);
  await page.screenshot({ path: path.join(output, "lookup-narrow.png") });
  const charge = facts(panel, "供应商实际扣费（credits）");
  await charge.scrollIntoViewIfNeeded();
  const chargeBounds = await charge.boundingBox();
  assert.ok(chargeBounds && chargeBounds.x >= 0 && chargeBounds.x + chargeBounds.width <= 390 && chargeBounds.y >= 0 && chargeBounds.y + chargeBounds.height <= 844);
  await page.screenshot({ path: path.join(output, "lookup-charge-narrow.png") });
  await page.setViewportSize({ width: 1440, height: 1100 });
  for (const status of [401, 429, 502]) {
    await query(status);
    await panel.getByText("任务查询失败，请检查账号、任务 ID 与配置版本后重试。", { exact: true }).waitFor();
    assert.equal(await panel.getByLabel("Luma 任务 ID", { exact: true }).count(), 0);
    assert.equal(await page.locator("#login-token").count(), 0);
    assert.equal(await read.isEnabled(), true);
    record(`error-${status}-keeps-management-session`);
  }
  const close = () => page.getByRole("dialog", { name: "编辑账号", exact: true }).getByRole("button", { name: "取消", exact: true }).click();
  await close(); await button("编辑账号").click();
  assert.equal(await input.inputValue(), "");
  assert.equal(await panel.getByLabel("Luma 任务 ID", { exact: true }).count(), 0);
  await close();
  await page.getByRole("combobox", { name: "切换账号", exact: true }).selectOption("luma-pipeline:unrelated");
  await button("编辑账号").click(); assert.equal(await input.inputValue(), "");
  assert.equal(await panel.getByText(/任务查询失败/).count(), 0);
  await close(); record("close-and-account-switch-clear");
  await button("运维").click(); await button(`请求详情: ${ready.auditId}`).click();
  await expectText(facts(audit, "状态"), "已取消");
  await expectText(facts(audit, "供应商实际扣费（credits）"), "未知");
  assert.equal(calls.length, 6);
  for (const call of calls) assert.deepEqual(call, { method: "POST", body: { providerId: ready.provider,
    credentialId: ready.credential, expectedRevision: ready.revision, generationId: taskId } });
  assert.deepEqual(errors, []); assert.deepEqual(external, []);
  assert.ok(sessions.some(item => item.path.endsWith("/confirm-secret-access") && item.status === 200));
  await writeFile(path.join(output, "final.ax.txt"), await page.locator("body").ariaSnapshot());
  record("original-audit-remains-unknown-cancelled"); success = true;
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
      browserClosed: !browser?.isConnected(), proxyClosed, origin: server?.origin, calls, responses, sessions, errors, external,
      checkpoints, scope: "normal main UI and management TCP; synthetic upstream only" }, null, 2) + "\n");
  }
}
console.log(JSON.stringify({ status: success ? "passed" : "failed", output, errors }));
