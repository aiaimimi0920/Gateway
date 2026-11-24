// Exercise only the new review flow in the real component, with synthetic local responses.
import assert from "node:assert/strict";
import { writeFile } from "node:fs/promises";
import path from "node:path";

export async function checkReview(page, dialog, output) {
  const field = name => dialog.getByLabel(name, { exact: true });
  const click = name => dialog.getByRole("button", { name, exact: true }).click();
  await field("提示词").fill("A blue circle");
  await field("会话预算上限（credits）").fill("20");
  await click("准备任务");
  await field("确认使用此 Luma 账号的 credits 生成").check();
  await click("提交生成"); await click("查询结果与费用");
  const review = dialog.getByRole("region", { name: "待审阅内容", exact: true });
  await review.waitFor();
  const image = review.getByRole("img", { name: "待审阅图片", exact: true });
  await image.scrollIntoViewIfNeeded();
  assert.equal(await image.evaluate(img => img.complete && img.naturalWidth > 0), true);
  assert.equal(await dialog.getByRole("button", { name: "提交审阅答复", exact: true }).isEnabled(), false);
  await page.screenshot({ path: path.join(output, "review-wide.png") });
  await field("审阅答复（JSON）").fill("{");
  await review.getByText("请输入有效 JSON，最大 16 KiB。", { exact: true }).waitFor();
  await page.setViewportSize({ width: 390, height: 844 });
  await field("审阅答复（JSON）").fill('{"decision":"accept"}');
  await field("审阅答复（JSON）").scrollIntoViewIfNeeded();
  const bounds = await field("审阅答复（JSON）").boundingBox();
  assert.ok(bounds && bounds.x >= 0 && bounds.x + bounds.width <= 390);
  const scroll = await review.evaluate(element => ({ width: element.clientWidth, scroll: element.scrollWidth }));
  assert.ok(scroll.scroll <= scroll.width + 1, JSON.stringify(scroll));
  await page.screenshot({ path: path.join(output, "review-narrow.png") });
  await writeFile(path.join(output, "review.ax.txt"), await dialog.ariaSnapshot());
  await field("已审阅内容，确认按此答复继续；可能使用剩余会话 credits").check();
  await click("提交审阅答复");
  await dialog.getByText("已收到答复，继续处理", { exact: true }).waitFor();
  assert.equal(await dialog.getByRole("button", { name: "提交审阅答复", exact: true }).count(), 0);
  await click("查询结果与费用");
  await dialog.getByText(/已完成本次审阅/).waitFor();
  const calls = await page.evaluate(() => window.syntheticRequests);
  assert.deepEqual(calls.map(call => call.body.action), ["prepare", "start", "read", "respond", "read"]);
  assert.deepEqual(calls[3].body.answer, { invocationId: "review-1", fingerprint: "synthetic-review-1", result: { decision: "accept" } });
  assert.equal(calls[3].body.chargeConfirmed, true);
  assert.equal(calls[3].body.request, undefined);
  return calls;
}
