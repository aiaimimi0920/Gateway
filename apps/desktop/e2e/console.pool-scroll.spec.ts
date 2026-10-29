import { expect, test, type Page } from "@playwright/test";
import { installConsoleApiMocks } from "./console.e2e.mocks";

async function openPools(page: Page, count: number) {
  const state = await installConsoleApiMocks(page);
  const providers = Array.from({ length: count }, (_, index) => ({
    id: `scroll-site-${index}`,
    label: `Scroll Site ${String(index).padStart(2, "0")}`,
    preset: "openai",
    base_url: `https://scroll-${index}.example.invalid`,
    supported_models: ["gpt-5.4"],
    credentials: [{ id: `account-${index}`, account_name: `Fixture ${index}`, api_key: "synthetic-key" }],
  }));
  state.routeConfig.routeConfig.document.providers = providers;
  // The account catalog is a separate read API; keep it consistent with the route fixture.
  await page.route("**/v1/internal/gateway/account-groups", (route) => route.fulfill({
    status: 200,
    contentType: "application/json",
    body: JSON.stringify({ summary: {
      routeConfigRevision: "r1-deadbeefcafe", source: "e2e", accountGroups: [],
      accounts: providers.map((provider, index) => ({
        id: `${provider.id}:account-${index}`, displayName: `Fixture ${index}`,
        providerId: provider.id, providerLabel: provider.label, providerPreset: provider.preset,
        credentialId: `account-${index}`, baseUrl: provider.base_url, mode: "credential",
        enabled: true, supportedModels: provider.supported_models, groupIds: [],
      })),
      providers: providers.map((provider, index) => ({
        id: provider.id, label: provider.label, preset: provider.preset, baseUrl: provider.base_url,
        accountIds: [`${provider.id}:account-${index}`], supportedModels: provider.supported_models,
      })),
    } }),
  }));
  await page.goto("/ui/");
  await page.getByLabel("管理密钥").fill("gateway-admin-token");
  await page.getByRole("button", { name: "登录" }).click();
  await page.getByRole("navigation", { name: "Gateway console navigation" })
    .getByRole("button", { name: /凭据池/ }).click();
}

test("appends pools on scroll while retaining the original card and its flipped state", async ({ page }, testInfo) => {
  await openPools(page, 49);
  const cards = page.locator("[data-provider-card]");
  await expect(cards).toHaveCount(24);
  await expect(page.getByRole("navigation", { name: "凭据池分页" })).toHaveCount(0);
  const first = cards.first();
  const original = await first.elementHandle();
  await first.getByRole("button", { name: "翻面查看 Scroll Site 00 详情" }).click();
  await expect(first).toHaveAttribute("data-provider-card-side", "back");
  await page.locator(".nt-pool-scroll-loader").scrollIntoViewIfNeeded();
  if (process.env.GATEWAY_E2E_SCROLL_DIAGNOSTICS === "1") {
    const geometry = await page.locator(".nt-pool-scroll-loader").evaluate((element) => {
      const ancestors = [];
      for (let parent: Element | null = element; parent; parent = parent.parentElement) {
        const style = getComputedStyle(parent);
        ancestors.push({ name: parent.className, overflow: style.overflowY, height: parent.clientHeight,
          scrollHeight: parent.scrollHeight, scrollTop: parent.scrollTop, rect: parent.getBoundingClientRect().toJSON() });
      }
      return { ancestors, scrollY, innerHeight, viewport: visualViewport?.height };
    });
    await testInfo.attach("scroll-geometry", { body: JSON.stringify(geometry, null, 2), contentType: "application/json" });
  }
  await expect(cards).toHaveCount(48);
  expect(await original!.evaluate((element) => element.isConnected)).toBe(true);
  await expect(first).toHaveAttribute("data-provider-card-side", "back");
  await page.locator(".nt-pool-scroll-loader").scrollIntoViewIfNeeded();
  await expect(cards).toHaveCount(49);
  await expect(page.locator(".nt-pool-scroll-loader")).toHaveCount(0);
  expect(await cards.evaluateAll((items) => new Set(items.map((item) => item.getAttribute("data-provider-card"))).size)).toBe(49);
  const search = page.getByRole("searchbox", { name: "搜索站点" });
  await search.fill("Scroll Site 48");
  await expect(cards).toHaveCount(1);
  await expect(cards.first()).toHaveAttribute("data-provider-card", "scroll-site-48");
  await search.fill("");
  await expect(cards).toHaveCount(24);
  await expect(cards.first()).toBeVisible();
  await page.getByRole("combobox", { name: "站点排序" }).click();
  await page.getByRole("option", { name: "名称降序" }).click();
  await expect(cards).toHaveCount(24);
  await expect(cards.first()).toHaveAttribute("data-provider-card", "scroll-site-48");
});

test("centers fixed-width tracks and keeps incomplete rows aligned after resizing", async ({ page }, testInfo) => {
  await openPools(page, 8);
  const grid = page.locator(".nt-provider-card-grid");
  const cards = page.locator("[data-provider-card]");
  await expect(cards).toHaveCount(8);
  for (const width of [1600, 1920, 1280, 1024]) {
    await page.setViewportSize({ width, height: 1000 });
    await expect.poll(() => grid.evaluate((element) => {
      const expected = Math.max(1, Math.floor((element.clientWidth + 16) / 308));
      return Number((element as HTMLElement).style.getPropertyValue("--nt-provider-grid-columns")) - expected;
    })).toBe(0);
    const layout = await grid.evaluate((element) => {
      const bounds = element.getBoundingClientRect();
      const styles = getComputedStyle(element);
      const columns = Number((element as HTMLElement).style.getPropertyValue("--nt-provider-grid-columns"));
      const rectangles = [...element.querySelectorAll("[data-provider-card]")].map((card) => {
        const box = card.getBoundingClientRect();
        return { left: box.left - bounds.left, width: box.width };
      });
      return { columns, gap: parseFloat(styles.columnGap), width: bounds.width, rectangles };
    });
    expect(layout.gap).toBeGreaterThanOrEqual(16);
    expect(layout.gap).toBeLessThanOrEqual(28);
    for (const box of layout.rectangles) expect(box.width).toBeCloseTo(292, 0);
    const left = layout.rectangles[0].left;
    const right = layout.width - layout.rectangles[layout.columns - 1].left - 292;
    expect(Math.abs(left - right)).toBeLessThanOrEqual(1);
    for (let index = layout.columns; index < layout.rectangles.length; index++) {
      expect(layout.rectangles[index].left).toBeCloseTo(layout.rectangles[index % layout.columns].left, 0);
    }
    if (width === 1600) await page.screenshot({ path: testInfo.outputPath("centered-original-cards.png") });
  }
  await cards.first().getByRole("button", { name: "显示 Scroll Site 00 账号库" }).click();
  const library = page.getByRole("region", { name: "Scroll Site 00 账号库" });
  await expect(library).toBeVisible();
  await expect(library.locator("[data-account-card]")).toHaveCount(1);
  await expect.poll(() => library.evaluate((element) => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
});
