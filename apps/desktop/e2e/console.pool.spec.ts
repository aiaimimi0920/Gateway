import { expect, test } from "@playwright/test";
import { installConsoleApiMocks } from "./console.e2e.mocks";

test.describe("Gateway web console provider pool", () => {
  test("flips a provider pool card while keeping the inactive face out of navigation", async ({
    page,
  }, testInfo) => {
    const state = await installConsoleApiMocks(page);
    const fixtureProvider = state.routeConfig.routeConfig.document.providers[0];
    if (!fixtureProvider || typeof fixtureProvider !== "object") throw new Error("Provider fixture is missing");
    Object.assign(fixtureProvider, {
      label: "NVIDIA", preset: "nvidia-openai", pool_target_size: 100,
    });

    await page.goto("/ui/");
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();
    await page
      .getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /凭据池/ })
      .click();

    const card = page.locator('[data-provider-card="managed-provider"]');
    const inner = card.locator(".nt-provider-card__inner");
    const front = card.locator(".nt-provider-card__front");
    const back = card.locator(".nt-provider-card__back");
    const flipButton = card.getByRole("button", { name: /翻面查看.*详情/ });
    const frontTransform = await inner.evaluate((element) => getComputedStyle(element).transform);

    await expect(card).toHaveAttribute("data-provider-card-side", "front");
    await expect(card).toHaveCSS("perspective", "1200px");
    await expect(front).toHaveAttribute("aria-hidden", "false");
    await expect(back).toHaveAttribute("aria-hidden", "true");
    await expect(front).toHaveCSS("backface-visibility", "hidden");
    await expect(back).toHaveCSS("backface-visibility", "hidden");
    await expect(flipButton).toBeVisible();
    await expect(flipButton.locator(".lucide-gallery-horizontal-end")).toBeVisible();
    await expect(front.locator("[data-provider-icon]")).toBeVisible();
    await expect(card.getByRole("img", { name: /可用 .*待恢复 .*失效 .*剩余/ })).toBeVisible();
    // This fixture has no historical windows; the front must not invent a chart.
    await expect(card.getByRole("img", { name: /最近窗口的调用成功率/ })).toHaveCount(0);
    await expect(card.getByRole("switch", { name: /调度开关/ })).toBeVisible();

    await flipButton.click();

    await expect(card).toHaveAttribute("data-provider-card-side", "back");
    await expect(front).toHaveAttribute("aria-hidden", "true");
    await expect(back).toHaveAttribute("aria-hidden", "false");
    await expect(card.getByRole("region", { name: /账号生命周期/ })).toBeVisible();
    await expect(card.getByText("可用池", { exact: true })).toBeVisible();
    await expect(card.getByText("冷却池", { exact: true })).toBeVisible();
    await expect(card.getByText("失效池", { exact: true })).toBeVisible();
    await expect(back.locator("[data-provider-icon]")).toHaveAttribute("data-provider-icon", "nvidia");
    await expect(back.locator("[data-provider-icon] svg")).toBeVisible();
    await expect(back.locator("[data-provider-icon]")).toHaveAttribute("data-provider-icon", await front.locator("[data-provider-icon]").getAttribute("data-provider-icon") ?? "");
    await expect(card.locator(".nt-provider-lifecycle__group")).toHaveCount(3);
    for (const label of ["最小可用池", "最大可用池", "存储路径", "存储密码", "归档路径", "归档密码"]) {
      await expect(card.getByRole("button", { name: new RegExp(`编辑 .* ${label}`) })).toBeVisible();
    }
    await expect(card.getByRole("button", { name: /复制 .* 补号通知 API/ })).toBeVisible();
    await expect(card.getByRole("button", { name: /复制 .* 信息查询 API/ })).toBeVisible();
    await expect(card.locator(".nt-provider-lifecycle__endpoint a")).toHaveCount(0);
    await expect(card.locator(".nt-provider-lifecycle__endpoint").first()).not.toContainText("/v1/internal");
    await expect(card.getByText("清空（4）", { exact: true })).toBeVisible();
    await expect(card.getByRole("button", { name: /翻回.*卡牌正面/ })).toBeFocused();
    await expect
      .poll(() => inner.evaluate((element) => getComputedStyle(element).transform))
      .not.toBe(frontTransform);
    const backBody = card.locator(".nt-provider-card__back-body");
    await expect(backBody).toHaveCSS("overflow-y", "visible");
    await expect
      .poll(() => backBody.evaluate((element) => element.scrollHeight - element.clientHeight))
      .toBeLessThanOrEqual(1);
    await expect
      .poll(() => backBody.evaluate((element) => element.scrollWidth - element.clientWidth))
      .toBeLessThanOrEqual(1);
    if (process.env.GATEWAY_E2E_CAPTURE_CARDS === "1") {
      await card.screenshot({
        animations: "disabled",
        path: `output/playwright/provider-card-back-${testInfo.project.name}.png`,
      });
    }

    await card.getByRole("button", { name: "编辑 NVIDIA 最大可用池" }).click();
    const maximum = card.getByRole("spinbutton", { name: "NVIDIA 最大可用池" });
    await maximum.fill("1000000");
    await maximum.press("Escape");
    await expect(maximum).toHaveCount(0);
    await expect(card.getByRole("button", { name: "编辑 NVIDIA 最大可用池" })).toBeFocused();
    await card.getByRole("button", { name: "编辑 NVIDIA 存储路径" }).click();
    await card.getByRole("textbox", { name: "NVIDIA 存储路径" }).fill("https://example.invalid/pool");
    await card.getByRole("button", { name: "保存 NVIDIA 存储路径" }).click();
    await expect(page.getByText(/HTTP\/HTTPS 云存储尚未配置/)).toBeVisible();
    await card.getByRole("button", { name: "取消编辑 NVIDIA 存储路径" }).click();
    await expect.poll(() => backBody.evaluate((element) => element.scrollHeight - element.clientHeight)).toBeLessThanOrEqual(1);
    await card.getByRole("button", { name: /翻回.*卡牌正面/ }).focus();

    await page.keyboard.press("Space");

    await expect(card).toHaveAttribute("data-provider-card-side", "front");
    await expect(flipButton).toBeFocused();
    await expect
      .poll(() => inner.evaluate((element) => getComputedStyle(element).transform))
      .toBe(frontTransform);
  });

  test("renders populated LongCat preview metrics from ten read-only demo accounts", async ({
    page,
  }, testInfo) => {
    const state = await installConsoleApiMocks(page);
    state.routeConfig.routeConfig.document.providers.push({
      id: "longcat",
      label: "LongCat",
      vendor_name: "LongCat",
      preset: "longcat-openai",
      base_url: "https://api.longcat.chat/openai",
      supported_models: ["LongCat-2.0"],
      credentials: [
        {
          id: "longcat-live",
          account_name: "LongCat Current",
          api_key: "fake-longcat-key",
        },
      ],
    });
    state.routeConfig.routeConfig.document.model_routes.push({
      pattern: "LongCat-*",
      provider_ids: ["longcat"],
    });

    await page.goto("/ui/");
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();
    await page
      .getByRole("navigation", { name: "Gateway console navigation" })
      .getByRole("button", { name: /凭据池/ })
      .click();

    const card = page.locator('[data-provider-card="longcat"]');
    await expect(card.getByText("7/30", { exact: true })).toBeVisible();
    await expect(
      card.getByRole("img", { name: /可用 7，待恢复 2，失效 2，剩余 19/ }),
    ).toBeVisible();
    await expect(card.locator('[data-provider-metric="concurrency"]')).toHaveText("30/100");
    await expect(card.locator('[data-provider-metric="upstream-cost"]')).toHaveText("$7.80");
    await expect(card.locator('[data-provider-metric="platform-revenue"]')).toHaveText("$13.20");
    await expect(card.locator('[data-provider-metric="requests"]')).toHaveText("980");
    await expect(card.locator('[data-provider-metric="success-rate"]')).toHaveText("90.3%");
    const metricRows = await card.locator(".nt-provider-card__metrics > div").evaluateAll((items) =>
      items.map((item) => Math.round(item.getBoundingClientRect().top)),
    );
    expect(new Set(metricRows).size).toBe(1);
    await expect(card.locator(".nt-provider-card__availability-window")).toHaveCount(4);
    await expect(card.locator(".nt-provider-card__availability-window").first()).toHaveAttribute(
      "title",
      "13:35：163/200 次成功（82%）",
    );
    await expect(card.locator(".nt-provider-card__availability-window").last()).toHaveAttribute(
      "title",
      "13:50：195/200 次成功（98%）",
    );
    await expect(card.locator("[data-provider-availability-cell]")).toHaveCount(48);
    await expect(card.locator('[data-provider-availability-cell="success"]')).toHaveCount(41);
    await expect(card.locator('[data-provider-availability-cell="mixed"]')).toHaveCount(4);
    await expect(card.locator('[data-provider-availability-cell="failure"]')).toHaveCount(3);
    await expect(
      card.getByRole("img", {
        name: /最近窗口的调用成功率，所有模型聚合，722\/800 次成功，90\.3%/,
      }),
    ).toBeVisible();
    if (process.env.GATEWAY_E2E_CAPTURE_CARDS === "1") {
      await card.screenshot({
        animations: "disabled",
        path: `output/playwright/provider-card-front-${testInfo.project.name}.png`,
      });
    }

    const accountLibraryButton = card.getByRole("button", { name: "显示 LongCat 账号库" });
    const flipButton = card.getByRole("button", { name: /翻面查看 LongCat 详情/ });
    await expect(accountLibraryButton).toHaveAttribute("aria-expanded", "false");
    expect(
      await accountLibraryButton.evaluate(
        (button, flip) =>
          Boolean(button.compareDocumentPosition(flip as Node) & Node.DOCUMENT_POSITION_FOLLOWING),
        await flipButton.elementHandle(),
      ),
    ).toBe(true);
    const cardBoxBeforeExpansion = await card.boundingBox();
    await accountLibraryButton.click();

    const library = page.getByRole("region", { name: "LongCat 账号库" });
    await expect(card.getByRole("button", { name: "收起 LongCat 账号库" })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    const libraryHandle = await library.elementHandle();
    const providerStack = card.locator("..");
    await expect(providerStack).toHaveAttribute("data-provider-card-stack", "longcat");
    expect(
      await providerStack.evaluate(
        (element, accountLibrary) =>
          element.nextElementSibling === accountLibrary,
        libraryHandle,
      ),
    ).toBe(true);
    const providerStackBox = await providerStack.boundingBox();
    const libraryBox = await library.boundingBox();
    const providerLibraryGap =
      (libraryBox?.y ?? 0) - ((providerStackBox?.y ?? 0) + (providerStackBox?.height ?? 0));
    expect(providerLibraryGap).toBeGreaterThanOrEqual(0);
    expect(providerLibraryGap).toBeLessThanOrEqual(12);
    const cardBoxAfterExpansion = await card.boundingBox();
    expect(cardBoxAfterExpansion?.width).toBeCloseTo(cardBoxBeforeExpansion?.width ?? 0, 0);
    expect(cardBoxAfterExpansion?.height).toBeCloseTo(cardBoxBeforeExpansion?.height ?? 0, 0);
    const pageGrid = library.locator("[data-account-library-page]");
    const pageSize = Number(await pageGrid.getAttribute("data-account-library-page-size"));
    expect(pageSize).toBeGreaterThan(0);
    await expect(library.locator("[data-account-card]")).toHaveCount(Math.min(pageSize, 11));
    await expect(library.getByText("LongCat Preview 01")).toBeVisible();
    await expect(library.getByRole("button", { name: "手动录入账号" })).toBeVisible();
    const firstAccountCard = library.locator("[data-account-card]").first();
    await expect(firstAccountCard.locator(".nt-provider-account-card__identity svg")).toHaveCount(0);
    await expect(firstAccountCard.getByRole("combobox", { name: /调整 .* 分组池/ })).toBeVisible();
    const accountCardWidths = await library.locator("[data-account-card]").evaluateAll((cards) =>
      cards.map((accountCard) => Math.round(accountCard.getBoundingClientRect().width)),
    );
    expect(new Set(accountCardWidths).size).toBe(1);
    expect(accountCardWidths[0]).toBeLessThanOrEqual(292);
    await firstAccountCard.getByRole("button", { name: /查看 .* 统计/ }).click();
    await expect(page.getByRole("dialog", { name: "查看账号统计" })).toBeVisible();
    await page
      .getByRole("dialog", { name: "查看账号统计" })
      .getByText("关闭", { exact: true })
      .click();
    const accountMoreButton = firstAccountCard.getByRole("button", { name: /更多操作/ });
    await accountMoreButton.click();
    const accountActionMenu = page.getByRole("menu");
    await expect(page.getByRole("menuitem", { name: "账号测试" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "复制账号" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "查看统计" })).toHaveCount(0);
    await expect(page.getByRole("menuitem", { name: "定时测试" })).toHaveCount(0);
    const accountMoreButtonBox = await accountMoreButton.boundingBox();
    const accountActionMenuBox = await accountActionMenu.boundingBox();
    const accountActionMenuGap =
      (accountMoreButtonBox?.y ?? 0) -
      ((accountActionMenuBox?.y ?? 0) + (accountActionMenuBox?.height ?? 0));
    expect(accountActionMenuGap).toBeGreaterThanOrEqual(0);
    expect(accountActionMenuGap).toBeLessThanOrEqual(12);
    await page.keyboard.press("Escape");
    await card.getByRole("button", { name: /LongCat 更多操作/ }).click();
    await expect(page.getByRole("menuitem", { name: "服务商测试" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "自动定时测试" })).toBeVisible();
    await expect(page.getByRole("menuitem", { name: "查看账号明细" })).toHaveCount(0);
    await expect(page.getByRole("menuitem", { name: "复制账号" })).toHaveCount(0);
    await page.keyboard.press("Escape");
    const pageInput = library.getByRole("spinbutton", { name: "跳转页数" });
    await pageInput.fill((await pageInput.getAttribute("max")) ?? "1");
    await pageInput.press("Enter");
    await expect(library.getByText("LongCat Preview 10")).toBeVisible();
    await expect
      .poll(() =>
        library.evaluate((element) => element.scrollWidth - element.clientWidth),
      )
      .toBeLessThanOrEqual(1);
    if (process.env.GATEWAY_E2E_CAPTURE_CARDS === "1") {
      await library.screenshot({
        path: `output/playwright/provider-account-library-${testInfo.project.name}.png`,
      });
    }
  });

  test("keeps the workspace scrollable at a tablet viewport", async ({ page }) => {
    await installConsoleApiMocks(page);
    await page.setViewportSize({ width: 1024, height: 720 });

    await page.goto("/ui/");
    await page.getByLabel("管理密钥").fill("gateway-admin-token");
    await page.getByRole("button", { name: "登录" }).click();
    await expect(
      page.getByRole("navigation", { name: "Gateway console navigation" }),
    ).toBeVisible();

    const stage = page.locator(".nt-stage");
    await expect.poll(async () => stage.evaluate((element) => element.clientHeight)).toBeGreaterThan(120);
    await stage.hover();
    await page.mouse.wheel(0, 600);
    await expect.poll(async () => stage.evaluate((element) => element.scrollTop)).toBeGreaterThan(0);
  });
});
