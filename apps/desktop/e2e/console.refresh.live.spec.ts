import { expect, test } from "@playwright/test";

test.describe.configure({ mode: "serial" });

test("keeps an unsaved group across language changes and discards it on refresh in a live console", async ({
  page,
}) => {
  const managementToken = process.env.GATEWAY_LIVE_MANAGEMENT_TOKEN;
  test.skip(!managementToken, "GATEWAY_LIVE_MANAGEMENT_TOKEN is required");

  await page.goto("/ui/");
  await expect(page.getByRole("heading", { name: "登录" })).toBeVisible();
  await page.getByLabel("管理密钥").fill(managementToken ?? "");
  await page.getByRole("button", { name: "登录" }).click();
  await expect(page.getByRole("heading", { name: "Gateway 网页控制台" })).toBeVisible();

  await page
    .getByRole("navigation", { name: "Gateway console navigation" })
    .getByRole("button", { name: /分组策略/ })
    .click();

  const existingGroupIds = page.getByLabel(/^分组 ID \d+$/);
  const existingGroupCount = await existingGroupIds.count();
  await page.getByRole("button", { name: "添加分组" }).click();

  const draftIndex = existingGroupCount + 1;
  const temporaryGroupId = `playwright-refresh-${Date.now()}`;
  await page.getByLabel(`分组 ID ${draftIndex}`).fill(temporaryGroupId);
  await expect(page.getByRole("status", { name: "Draft status" })).toContainText(
    "有未保存修改",
  );

  await page.getByRole("button", { name: "切换界面语言" }).click();
  await expect(page.getByLabel(`Group ID ${draftIndex}`)).toHaveValue(temporaryGroupId);
  await expect(page.getByRole("status", { name: "Draft status" })).toContainText(
    "Unsaved changes",
  );

  await page.getByRole("button", { name: "Refresh" }).click();
  const discardDialog = page.getByRole("dialog", { name: "Discard unsaved changes" });
  await expect(discardDialog).toBeVisible();
  await discardDialog.getByRole("button", { name: "Discard and refresh" }).click();
  await expect(page.getByText(temporaryGroupId, { exact: true })).toHaveCount(0);
  await expect(page.getByRole("status", { name: "Draft status" })).toContainText(
    "Draft in sync",
  );
  await expect(page.getByLabel(/^Group ID \d+$/)).toHaveCount(existingGroupCount);
});
