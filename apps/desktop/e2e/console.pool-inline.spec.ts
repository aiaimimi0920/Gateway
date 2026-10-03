import { expect, test, type Locator } from "@playwright/test";
import { mkdir, writeFile } from "node:fs/promises";
import { installConsoleApiMocks } from "./console.e2e.mocks";

const storagePath = `C:\\Gateway\\credentials\\${"provider-directory\\".repeat(12)}pool`;
const archivePath = `/srv/gateway/${"recovery-archive/".repeat(12)}pool`;
const notification = `https://example.invalid/${"refill/".repeat(40)}tasks/claim`;
const inquiry = `https://example.invalid/${"inquiry/".repeat(40)}provider`;

async function geometry(card: Locator, neighbor: Locator) {
  return {
    card: await card.boundingBox(),
    neighbor: await neighbor.boundingBox(),
    rows: await card.locator(".nt-provider-lifecycle__field").evaluateAll((rows) => rows.map((row) => {
      const { x, y, width, height } = row.getBoundingClientRect();
      return { x, y, width, height };
    })),
  };
}

for (const width of [undefined, 320, 1024]) {
  test(`keeps inline editors and complete endpoint copies at ${width ?? "default"} width`, async ({ page }, testInfo) => {
    if (width) await page.setViewportSize({ width, height: 900 });
    const state = await installConsoleApiMocks(page);
    const fixture = state.routeConfig.routeConfig.document.providers[0];
    if (!fixture || typeof fixture !== "object") throw new Error("Provider fixture is missing");
    Object.assign(fixture, {
      label: "NVIDIA", preset: "nvidia-openai", pool_target_size: 100,
      credential_storage_path: storagePath, credential_archive_path: archivePath,
    });
    state.routeConfig.routeConfig.document.providers.push({
      id: "longcat", label: "LongCat", vendor_name: "LongCat", preset: "longcat-openai",
      base_url: "https://api.longcat.chat/openai", supported_models: ["LongCat-2.0"],
      credentials: [{ id: "longcat-live", account_name: "LongCat Current", api_key: "fake-longcat-key" }],
    });
    state.routeConfig.routeConfig.document.model_routes.push({ pattern: "LongCat-*", provider_ids: ["longcat"] });
    await page.route("**/v1/internal/gateway/credential-pool-refill", (route) => route.fulfill({
      json: { refill: {
        enabled: true, streamKey: "synthetic-refill", notificationIntervalSeconds: 60,
        defaultLeaseSeconds: 120, maxLeaseSeconds: 600, revisionId: "r1-deadbeefcafe",
        providers: [{
          providerId: "managed-provider", providerLabel: "NVIDIA", targetSize: 100,
          credentialCount: 1, activeCredentialCount: 1, deficit: 99, needsRefill: true,
          autoRefillEnabled: false, directDriverConfigured: false, notificationEnabled: true,
          inquiryEnabled: true, userRequestEnabled: true, outstandingTaskId: null,
          outstandingTaskState: null, notificationApi: notification, inquiryApi: inquiry,
          credentialStoragePath: storagePath, storagePasswordConfigured: true,
          archiveStoragePath: archivePath, archivedCredentialCount: 4,
          permanentDeleteEnabled: false, revisionId: "r1-deadbeefcafe",
        }], recentTasks: [],
      } },
    }));
    await page.addInitScript(() => {
      Object.defineProperty(navigator, "clipboard", { value: {
        writeText: async (value: string) => { (window as unknown as { copied: string }).copied = value; },
      } });
    });
    await page.goto("/ui/");
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();
    await page.getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /凭据池/ }).click();
    const card = page.locator('[data-provider-card="managed-provider"]');
    const neighbor = page.locator('[data-provider-card="longcat"]');
    await card.getByRole("button", { name: /翻面查看.*详情/ }).click();
    await expect(card).toHaveAttribute("data-provider-card-side", "back");
    await expect(card.getByRole("button", { name: /翻回.*卡牌正面/ })).toBeFocused();
    await card.screenshot({ animations: "disabled", path: testInfo.outputPath("before-edit.png") });
    const evidence: { label: string; before: Awaited<ReturnType<typeof geometry>>; during: Awaited<ReturnType<typeof geometry>>; after: Awaited<ReturnType<typeof geometry>>; alignment: number[] }[] = [];
    for (const [label, draft] of [
      ["最小可用池", "0"], ["最大可用池", "99"], ["存储路径", storagePath],
      ["归档路径", archivePath], ["存储密码", "synthetic-password"],
    ]) {
      await card.scrollIntoViewIfNeeded();
      const before = await geometry(card, neighbor);
      const edit = card.getByRole("button", { name: `编辑 NVIDIA ${label}` });
      await edit.click();
      const input = card.getByLabel(`NVIDIA ${label}`, { exact: true });
      await expect(input).toBeFocused();
      expect((await input.boundingBox())?.width).toBeGreaterThanOrEqual(label.includes("可用池") ? 48 : 40);
      expect(await input.evaluate((element) => {
        const style = getComputedStyle(element);
        return element.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
      })).toBeGreaterThanOrEqual(20);
      await input.fill(draft);
      const field = card.locator(".nt-provider-lifecycle__field--editing");
      expect(await field.evaluate((row) => {
        const bounds = row.getBoundingClientRect();
        return Array.from(row.children).every((child) => {
          const rect = child.getBoundingClientRect();
          return rect.x >= bounds.x - 1 && rect.right <= bounds.right + 1;
        });
      })).toBe(true);
      const alignment = await field.evaluate((row) => Array.from(row.children)
        .filter((child) => child.tagName !== "SPAN" || !child.hasAttribute("role"))
        .map((child) => { const rect = child.getBoundingClientRect(); return rect.y + rect.height / 2; }));
      const during = await geometry(card, neighbor);
      await card.screenshot({ animations: "disabled", path: testInfo.outputPath(`editing-${label}.png`) });
      await input.press("Escape");
      await expect(edit).toBeFocused();
      evidence.push({ label, before, during, after: await geometry(card, neighbor), alignment });
      if (label === "存储密码") {
        await edit.click();
        await expect(input).toHaveValue("");
        await expect(input).toHaveAttribute("type", "password");
        await card.getByRole("button", { name: `取消编辑 NVIDIA ${label}` }).click();
        await expect(edit).toBeFocused();
        await expect(card).not.toContainText(draft);
      }
    }
    await writeFile(testInfo.outputPath("geometry.json"), JSON.stringify(evidence, null, 2));
    await testInfo.attach("geometry", { path: testInfo.outputPath("geometry.json"), contentType: "application/json" });
    if (process.env.GATEWAY_E2E_CAPTURE_CARDS === "1") {
      const destination = `output/playwright/inline-edit/${testInfo.project.name}`;
      await mkdir(destination, { recursive: true });
      await writeFile(`${destination}/geometry.json`, JSON.stringify(evidence, null, 2));
      await card.screenshot({ animations: "disabled", path: `${destination}/card.png` });
    }
    for (const item of evidence) {
      expect(item.during, item.label).toEqual(item.before);
      expect(item.after, item.label).toEqual(item.before);
      expect(Math.max(...item.alignment) - Math.min(...item.alignment), item.label).toBeLessThanOrEqual(1);
    }
    for (const [label, value] of [["补号通知", notification], ["信息查询", inquiry]]) {
      const endpoint = card.locator(".nt-provider-lifecycle__endpoint").filter({ hasText: label });
      const display = endpoint.locator(".nt-provider-lifecycle__field-value");
      await expect(display).toHaveText(value);
      await expect(display).toHaveCSS("text-overflow", "ellipsis");
      await expect(display).toHaveCSS("white-space", "nowrap");
      expect(await display.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true);
      await endpoint.getByRole("button", { name: `复制 NVIDIA ${label} API` }).click();
      expect(await page.evaluate(() => (window as unknown as { copied: string }).copied)).toBe(value);
    }
    await expect(card.locator(".nt-provider-lifecycle__endpoint a")).toHaveCount(0);
    expect(await card.evaluate((element) => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
    const minimumEdit = card.getByRole("button", { name: "编辑 NVIDIA 最小可用池" });
    await minimumEdit.click();
    const minimum = card.getByRole("spinbutton", { name: "NVIDIA 最小可用池" });
    const beforeError = await geometry(card, neighbor);
    await minimum.fill("-1");
    await minimum.press("Enter");
    await expect(card.getByRole("alert")).toBeVisible();
    await expect(minimum).toHaveAttribute("aria-invalid", "true");
    await card.screenshot({ animations: "disabled", path: testInfo.outputPath("validation-error.png") });
    expect(await card.getByRole("button", { name: "复制 NVIDIA 补号通知 API" }).evaluate((button) => {
      const rect = button.getBoundingClientRect();
      return button.contains(document.elementFromPoint(rect.x + rect.width / 2, rect.y + rect.height / 2));
    })).toBe(true);
    expect(await geometry(card, neighbor)).toEqual(beforeError);
    await minimum.fill("0");
    await minimum.press("Enter");
    await expect(minimumEdit).toBeFocused();
    await card.getByRole("button", { name: "编辑 NVIDIA 存储路径" }).click();
    await card.getByLabel("NVIDIA 存储路径", { exact: true }).fill(`${storagePath}-saved`);
    await card.getByRole("button", { name: "保存 NVIDIA 存储路径" }).click();
    await expect(card.locator(".nt-provider-lifecycle__field-value").filter({ hasText: `${storagePath}-saved` })).toHaveCount(1);
    await expect(card.getByRole("button", { name: "编辑 NVIDIA 存储路径" })).toBeFocused();
  });
}
