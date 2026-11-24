// Exercise explicit text testing through the normal main UI and a synthetic-only Rust host.
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
assert.equal(ready.scenario, "luma-inference-normal-main-v1");
await mkdir(path.dirname(output), { recursive: true });
await mkdir(output);
const require = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
const { chromium } = require("playwright");
const calls = [], errors = [], external = [], checkpoints = [], sessionResponses = [];
let server, browser, page, result, success = false, proxyClosed = false;
const endpoint = `/v1/internal/gateway/console/providers/${ready.provider}/probe`;
const button = name => page.getByRole("button", { name, exact: true });
const record = name => checkpoints.push({ name, probeRequests: calls.length });
async function configure() {
  await button("测试账号 exact").click();
  const inherit = page.getByRole("checkbox", { name: "继承", exact: true });
  await inherit.uncheck();
  assert.equal(await page.getByRole("checkbox", { name: "启用自动测试", exact: true }).isChecked(), false);
  await button("清空测试集").click();
  assert.equal(await button("运行测试").isDisabled(), true);
  await page.getByRole("textbox", { name: "临时提示词", exact: true }).fill("合成问题");
  await button("加入测试集").click();
  await button("清空选择").click();
  await page.getByRole("textbox", { name: "添加测试模型", exact: true }).fill(ready.model);
  await button("添加模型").click();
}
try {
  server = await serveMain(values.ui, ready.origin);
  browser = await chromium.launch({ channel: values.channel, headless: true });
  const context = await browser.newContext({ locale: "zh-CN", timezoneId: "UTC", viewport: { width: 1440, height: 1100 }, serviceWorkers: "block" });
  await context.route("**/*", route => {
    if (new URL(route.request().url()).origin === server.origin) return route.continue();
    external.push(new URL(route.request().url()).origin);
    return route.abort();
  });
  page = await context.newPage();
  page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => {
    if (new URL(request.url()).pathname === endpoint) calls.push(request.postDataJSON());
  });
  page.on("response", response => {
    if (response.url().includes("/console/session/")) sessionResponses.push({ path: new URL(response.url()).pathname, status: response.status() });
  });
  await page.goto(server.origin + "/ui/");
  await page.locator("#login-token").fill("console-management-contract-token");
  await button("登录").click();
  await button("凭据池").click();
  await button("exact").click();
  await button("账号配置").click();
  await configure();
  assert.equal(calls.length, 0);
  await button("关闭").click();
  assert.equal(calls.length, 0);
  record("configured-and-closed-without-send");
  await configure();
  await page.screenshot({ path: path.join(output, "configured.png") });
  const responseReady = page.waitForResponse(response => new URL(response.url()).pathname === endpoint);
  await button("运行测试").click();
  const response = await responseReady;
  assert.equal(response.status(), 200);
  result = await response.json();
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].credentialIds, ["exact"]);
  assert.deepEqual(calls[0].scope, { kind: "account", id: "exact" });
  assert.deepEqual(calls[0].testPlan.models, [ready.model]);
  assert.deepEqual(calls[0].testPlan.cases.filter(item => item.enabled).map(item => item.prompt), ["合成问题"]);
  assert.equal(result.result.results.length, 1);
  assert.equal(result.result.results[0].status, "passed");
  assert.deepEqual(result.result.results[0].assessment.models.map(item => item.model), [ready.model]);
  assert.ok(!JSON.stringify(result).includes("inference-synthetic-exact"));
  await button("运行测试").waitFor();
  const results = page.getByRole("region", { name: "测试结果", exact: true });
  await results.getByText("可联通调用", { exact: true }).waitFor();
  await results.getByText("答案详情", { exact: true }).click();
  const answer = results.getByText("实际回复：合成答案", { exact: true });
  await answer.scrollIntoViewIfNeeded();
  assert.equal(await answer.isVisible(), true);
  await page.screenshot({ path: path.join(output, "result.png") });
  await page.setViewportSize({ width: 390, height: 844 });
  await answer.scrollIntoViewIfNeeded();
  assert.equal(await answer.isVisible(), true);
  const bounds = await answer.boundingBox();
  assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390 && bounds.y >= 0 && bounds.y + bounds.height <= 844);
  await page.screenshot({ path: path.join(output, "result-narrow.png") });
  await writeFile(path.join(output, "result.ax.txt"), await page.locator("body").ariaSnapshot());
  record("explicit-test-passed");
  await button("关闭").click();
  assert.equal(calls.length, 1);
  assert.ok(sessionResponses.some(item => item.path.endsWith("/confirm-secret-access") && item.status === 200));
  assert.deepEqual(errors, []);
  assert.deepEqual(external, []);
  success = true;
} catch (error) {
  errors.push(error.stack ?? String(error));
  if (page && !page.isClosed()) {
    await writeFile(path.join(output, "failure.ax.txt"), await page.locator("body").ariaSnapshot());
    await page.screenshot({ path: path.join(output, "failure.png") });
  }
  process.exitCode = 1;
} finally {
  try { await browser?.close(); await server?.close(); proxyClosed = true; }
  catch (error) { success = false; process.exitCode = 1; errors.push(error.stack ?? String(error)); }
  finally {
    await writeFile(path.join(host, "browser-command.tmp"), JSON.stringify({ action: success ? "finish" : "abort" }));
    await rename(path.join(host, "browser-command.tmp"), path.join(host, "command.json"));
    await writeFile(path.join(output, "browser-result.json"), JSON.stringify({ status: success ? "passed" : "failed",
      browserClosed: !browser?.isConnected(), proxyClosed, calls, result, sessionResponses, errors, external, checkpoints,
      scope: "normal main UI to management TCP; synthetic inference only" }, null, 2) + "\n");
  }
}
console.log(JSON.stringify({ status: success ? "passed" : "failed", output, errors }));
