import { expect, test } from "@playwright/test";
import { installConsoleApiMocks } from "./console.e2e.mocks";

test("preserves model-card layout, keyboard focus and attached account filtering", async ({
  page,
}, testInfo) => {
  const state = await installConsoleApiMocks(page);
  const model = "refactor-model";
  state.routeConfig.routeConfig.document = {
    providers: ["first", "second"].map((id) => ({
      id, label: `Fixture ${id}`, preset: "openai",
      base_url: `https://${id}.example.invalid`, supported_models: [model],
      credentials: [{ id: `${id}-credential`, account_name: `Fixture ${id} account` }],
    })),
    model_routes: [{ pattern: model, provider_ids: ["first", "second"] }],
    aliases: {},
  };
  // The console's account catalog comes from the summary API, independently of drafts.
  await page.route("**/v1/internal/gateway/account-groups", async (route) => {
    await route.fulfill({
      json: {
        summary: {
          routeConfigRevision: "r1-deadbeefcafe", source: "e2e", accountGroups: [],
          accounts: ["first", "second"].map((id) => ({
            id: `${id}:${id}-credential`, displayName: `Fixture ${id} account`,
            providerId: id, providerLabel: `Fixture ${id}`, providerPreset: "openai",
            credentialId: `${id}-credential`, baseUrl: `https://${id}.example.invalid`,
            mode: "credential", enabled: true, supportedModels: [model], groupIds: [],
          })),
          providers: ["first", "second"].map((id) => ({
            id, label: `Fixture ${id}`, preset: "openai",
            baseUrl: `https://${id}.example.invalid`,
            accountIds: [`${id}:${id}-credential`], supportedModels: [model],
          })),
        },
      },
    });
  });
  await page.goto("/ui/");
  await page.getByLabel("管理密钥").fill("gateway-admin-token");
  await page.getByRole("button", { name: "登录" }).click();
  await page.getByRole("navigation", { name: "Gateway console navigation" })
    .getByRole("button", { name: /模型池/ }).click();

  const card = page.locator(`[data-model-pool-card="${model}"]`);
  const front = card.locator(".nt-entitlement-group-card__front");
  const back = card.locator(".nt-entitlement-group-card__back");
  await expect(card).toHaveAttribute("data-model-pool-card-side", "front");
  await expect(card.locator('[data-model-pool-metric="providers"]')).toHaveText("2");
  await expect(card.locator('[data-model-pool-metric="accounts"]')).toHaveText("2");
  await expect(back).toHaveAttribute("inert", "");
  await expect(front).toHaveCSS("backface-visibility", "hidden");
  await card.screenshot({ path: testInfo.outputPath("model-pool-front.png"), animations: "disabled" });
  const initialBox = await card.boundingBox();

  const menuTrigger = card.getByRole("button", { name: `${model} 更多操作` });
  await menuTrigger.click();
  await expect(card.getByRole("menu")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menuTrigger).toBeFocused();
  await expect(card.getByRole("menu")).toHaveCount(0);
  await card.getByRole("button", { name: `显示 ${model} 可用账号` }).click();
  const accounts = page.getByRole("region", { name: `${model} 可用账号` });
  await expect(accounts.locator("[data-account-card]")).toHaveCount(2);
  const expandedBox = await card.boundingBox();
  expect(expandedBox?.width).toBeCloseTo(initialBox?.width ?? 0, 0);
  expect(expandedBox?.height).toBeCloseTo(initialBox?.height ?? 0, 0);

  await card.getByRole("button", { name: `翻面调整 ${model} 优先级` }).click();
  await expect(card).toHaveAttribute("data-model-pool-card-side", "back");
  await expect(front).toHaveAttribute("inert", "");
  await expect(card.getByRole("button", { name: `翻回 ${model} 卡牌正面` })).toBeFocused();
  await expect(back).toHaveAttribute("aria-hidden", "false");
  await card.screenshot({ path: testInfo.outputPath("model-pool-back.png"), animations: "disabled" });
  await card.getByRole("button", { name: "Fixture first", exact: true }).click();
  await expect(accounts.locator("[data-account-card]")).toHaveCount(1);
  await expect(accounts.getByText("Fixture second account", { exact: true })).toBeVisible();
  await card.getByRole("button", { name: `翻回 ${model} 卡牌正面` }).focus();
  await page.keyboard.press("Space");
  await expect(card).toHaveAttribute("data-model-pool-card-side", "front");
  await expect(card.getByRole("button", { name: `翻面调整 ${model} 优先级` })).toBeFocused();
});
