import { expect, test } from "@playwright/test";
import { installConsoleApiMocks } from "./console.e2e.mocks";

for (const locale of ["zh-CN", "en-US"] as const) {
  test(`keeps the credential pool controls on one usable row (${locale})`, async ({ page }) => {
    await page.setViewportSize({ width: 1360, height: 820 });
    await page.addInitScript((value) => localStorage.setItem("gateway-ui-locale", value), locale);
    await installConsoleApiMocks(page);
    await page.goto("/ui/");
    await page.getByLabel(/管理密钥|Management token/i).fill("gateway-admin-token");
    await page.getByRole("button", { name: /^(登录|Sign in)$/i }).click();
    await page.getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /凭据池|Credential pool/i }).click();

    const search = page.getByRole("searchbox", { name: /搜索站点|Search sites/ });
    const sort = page.getByRole("combobox", { name: /站点排序|Sort sites/ });
    const addProvider = page.getByRole("button", { name: /^(添加服务商|Add provider)$/ });
    const addAccount = page.getByRole("button", { name: /^(添加账号|Add account)$/ });
    const controls = [search, sort, addProvider, addAccount];
    for (const control of controls) await expect(control).toHaveCount(1);
    await sort.press("Enter");
    const sortMenu = page.getByRole("listbox");
    await expect(sortMenu).toBeVisible();
    await sortMenu.getByRole("option", { name: /可用账号最多|Most available accounts/ }).click();
    await search.fill("fixture");

    // Match the titlebar reservation applied when the native controls are present.
    await page.evaluate(() => document.body.append(Object.assign(document.createElement("div"), {
      className: "nt-native-controls",
    })));
    for (const width of [1360, 1024, 900, 845, 750, 670, 481, 480, 390, 320]) {
      await page.setViewportSize({ width, height: 820 });
      for (const [index, control] of controls.entries()) {
        expect(await control.evaluate((element) => {
          const rect = element.getBoundingClientRect();
          const hit = document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2);
          return rect.width > 0 && rect.x >= 0 && rect.right <= innerWidth && element.contains(hit);
        }), `control ${index + 1} is not reachable at ${width}px`).toBe(true);
      }
      const textWidth = await search.evaluate((input) => {
        const { paddingLeft, paddingRight } = getComputedStyle(input);
        return input.clientWidth - parseFloat(paddingLeft) - parseFloat(paddingRight);
      });
      expect(textWidth, `search text is squeezed at ${width}px`).toBeGreaterThanOrEqual(72);
      const boxes = await Promise.all(controls.map((control) => control.boundingBox()));
      const centers = boxes.map((box) => box!.y + box!.height / 2);
      expect(Math.max(...centers) - Math.min(...centers), `controls wrap at ${width}px`).toBeLessThanOrEqual(2);
    }
    await expect(search).toHaveValue("fixture");
    await expect(search).toBeFocused();
    await page.getByRole("button", { name: /收起侧栏|Collapse sidebar/ }).click();
    await expect(page.getByRole("button", { name: /展开侧栏|Expand sidebar/ })).toBeFocused();
    for (const control of controls) await expect(control).toBeInViewport({ ratio: 1 });
    await expect(search).toHaveValue("fixture");
    await search.fill("");
    await expect(sort).toHaveAttribute("title", /可用账号最多|Most available accounts/);
    await sort.press("Enter");
    await expect(sortMenu).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(sort).toBeFocused();
    await addProvider.click();
    const providerDialog = page.getByRole("dialog", { name: /添加服务商|Add provider/ });
    await expect(providerDialog).toBeVisible();
    await providerDialog.getByRole("button", { name: /^(关闭|Close)$/ }).click();
    await expect(providerDialog).toBeHidden();
    await addAccount.click();
    await expect(page.getByRole("dialog", { name: /新增账号|Add account/ })).toBeVisible();
  });
}
