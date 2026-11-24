// Verify creation in the normal management UI. Only the Rust loopback fixture may receive API traffic.
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import path from "node:path";
import { serveMain } from "./normal-main-server.mjs";

const [fixtureOrigin, ui, output] = process.argv.slice(2);
if (!fixtureOrigin || !ui || !output) throw new Error("Expected <fixture-origin> <normal-main-build> <new-output-directory>");
await mkdir(output);
const require = createRequire(new URL("../../apps/desktop/package.json", import.meta.url));
const { chromium } = require("playwright");
const creationPath = "/v1/internal/gateway/console/luma-mcp-creation";
const isCreation = url => new URL(url).pathname === creationPath;
const calls = [], checkpoints = [], errors = [], external = [];
let server, browser, page, taskId, revision, passed = false;
const promptText = "a blue circle";
const asset = "https://cdn.example.test/creation.png?fixture=1";
const button = (scope, name) => scope.getByRole("button", { name, exact: true });
async function fixtureState() {
  const response = await fetch(new URL("/__fixture/luma-mcp-creation", fixtureOrigin), {
    redirect: "error", signal: AbortSignal.timeout(5000),
  });
  assert.equal(response.status, 200);
  const status = await response.json();
  assert.equal(status.scenario, "luma-mcp-creation-v1");
  return status;
}
async function checkpoint(name, posts, requests) {
  const state = await fixtureState();
  assert.equal(state.revision, revision);
  assert.equal(state.posts, posts, name);
  assert.equal(calls.length, requests, name);
  checkpoints.push({ name, posts, requests });
}
async function creationAction(panel, name) {
  const [response] = await Promise.all([
    page.waitForResponse(res => isCreation(res.url())), button(panel, name).click(),
  ]);
  assert.equal(response.status(), 200, name);
  assert.equal(await response.finished(), null, name);
}

try {
  // serveMain validates the fixture origin before any request is made.
  server = await serveMain(ui, fixtureOrigin);
  ({ revision } = await fixtureState());
  assert.ok(typeof revision === "string" && revision.length > 0);
  await checkpoint("fixture-ready", 0, 0);
  browser = await chromium.launch({ channel: "msedge", headless: true });
  const context = await browser.newContext({ locale: "zh-CN", timezoneId: "UTC",
    viewport: { width: 1440, height: 1100 }, serviceWorkers: "block" });
  await context.route("**/*", route => {
    const origin = new URL(route.request().url()).origin;
    if (origin === server.origin) return route.continue();
    external.push(origin);
    return route.abort();
  });
  page = await context.newPage();
  page.setDefaultTimeout(15_000);
  page.on("pageerror", error => errors.push(error.message));
  page.on("request", request => {
    if (isCreation(request.url())) calls.push(request.postDataJSON());
  });
  const account = page.getByRole("dialog", { name: "Luma MCP · 重新授权", exact: true });
  const panel = account.getByRole("region", { name: "Luma MCP 创作", exact: true });
  const id = panel.getByLabel("任务 ID", { exact: true });
  const prepare = "准备任务", read = "查询结果与费用";
  await page.goto(server.origin + "/ui/");
  await page.locator("#login-token").fill("console-management-contract-token");
  // Normal management login automatically obtains secret access; this must not prepare a task.
  const [grant] = await Promise.all([
    page.waitForResponse(res => new URL(res.url()).pathname.endsWith("/session/confirm-secret-access")),
    button(page, "登录").click(),
  ]);
  assert.equal(grant.status(), 200);
  assert.equal(await grant.finished(), null);
  await checkpoint("login-grant-does-not-prepare-or-submit", 0, 0);
  await button(page, "凭据池").click();
  await button(page, "saved-mcp").click();
  await button(page, "编辑账号").click();
  await panel.waitFor();
  await checkpoint("opening-does-not-submit", 0, 0);
  await panel.getByLabel("提示词", { exact: true }).fill(promptText);
  await panel.getByLabel("会话预算上限（credits）", { exact: true }).fill("20");

  await creationAction(panel, prepare);
  await panel.getByText("待提交", { exact: true }).waitFor();
  taskId = await id.inputValue();
  assert.match(taskId, /[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i);
  const submit = button(panel, "提交生成");
  assert.equal(await submit.isDisabled(), true);
  await checkpoint("prepared-without-charge-or-mcp", 0, 1);
  await panel.getByRole("checkbox", { name: "确认使用此 Luma 账号的 credits 生成", exact: true }).check();
  await creationAction(panel, "提交生成");
  await panel.getByText("生成中", { exact: true }).waitFor();
  await checkpoint("one-session-and-one-generation", 6, 2);

  await button(account, "关闭").last().click();
  await account.waitFor({ state: "hidden" });
  await button(page, "编辑账号").click();
  await panel.waitFor();
  assert.equal(await id.inputValue(), "");
  await checkpoint("reopen-does-not-resubmit", 6, 2);
  await id.fill(taskId);
  await creationAction(panel, read);
  await panel.getByText("上游已完成", { exact: true }).waitFor();
  await panel.getByText("会话费用：9 credits · 已结算", { exact: true }).waitFor();
  assert.equal(await panel.getByRole("link", { name: "打开生成结果", exact: true }).getAttribute("href"), asset);
  await checkpoint("same-task-result-and-session-cost", 15, 3);

  assert.deepEqual(calls.map(call => [call.action, call.taskId, call.chargeConfirmed]), [
    ["prepare", undefined, undefined], ["start", taskId, true], ["read", taskId, undefined],
  ]);
  assert.deepEqual(calls[0].request, calls[1].request);
  assert.equal(calls[0].request.prompt, promptText);
  assert.equal(calls[0].request.budgetCredits, 20);
  assert.equal(calls[2].request, undefined);
  for (const call of calls) {
    assert.equal(call.providerId, "luma-mcp");
    assert.equal(call.credentialId, "saved-mcp");
    assert.equal(call.expectedRevision, revision);
  }
  assert.deepEqual(errors, []);
  assert.deepEqual(external, []);
  passed = true;
} catch (error) {
  errors.push(error.stack ?? String(error));
  if (page && !page.isClosed()) {
    try { await writeFile(path.join(output, "failure.ax.txt"), await page.locator("body").ariaSnapshot()); }
    catch (snapshotError) { errors.push(String(snapshotError)); }
  }
} finally {
  const cleanup = await Promise.allSettled([browser?.close(), server?.close()]);
  for (const result of cleanup) if (result.status === "rejected") errors.push(String(result.reason));
  passed &&= errors.length === 0 && external.length === 0;
  await writeFile(path.join(output, "result.json"), JSON.stringify({
    status: passed ? "passed" : "failed", taskId, checkpoints, errors, external,
    browserClosed: !browser?.isConnected(), proxyClosed: cleanup[1].status === "fulfilled",
  }, null, 2) + "\n");
  if (!passed) { console.error(errors.join("\n")); process.exitCode = 1; }
}
