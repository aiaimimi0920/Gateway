import { chromium } from "playwright";

const baseUrl = process.env.PROBE_BASE_URL ?? "http://localhost:4200/ui/";
const token = process.env.PROBE_TOKEN ?? "123456";

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
await page.goto(baseUrl, { waitUntil: "domcontentloaded" });

await page.getByLabel("管理密钥").fill(token);
await page.getByRole("button", { name: "登录" }).click();
await page.getByRole("button", { name: "模型池", exact: true }).click();
await page.waitForTimeout(1500);

// The reorder path only exists on models more than one provider serves, so pick
// the widest chain in the pool rather than the first card.
const ranked = await page.evaluate(() =>
  [...document.querySelectorAll("[data-model-pool-card]")]
    .map((node) => ({
      model: node.getAttribute("data-model-pool-card"),
      providers: Number(
        node.querySelector('[data-model-pool-metric="providers"]')?.textContent ?? "0",
      ),
    }))
    .sort((left, right) => right.providers - left.providers)
    .slice(0, 6),
);
console.log("widest chains:", JSON.stringify(ranked));

const target = ranked[0];
if (!target || target.providers < 2) {
  console.log("no model has more than one provider; reorder path not exercisable");
  await browser.close();
  process.exit(0);
}

const card = page.locator(`[data-model-pool-card="${target.model}"]`);
await card.scrollIntoViewIfNeeded();
const back = card.locator(".nt-entitlement-group-card__back");
await card.locator(".nt-entitlement-group-card__front .nt-entitlement-group-card__flip").click();
await page.waitForTimeout(700);

console.log("model:", target.model);
console.log("chain links on page 1:", await back.locator("[data-model-chain-provider]").count());
console.log("pagers:", await back.locator(".nt-entitlement-scope__pager").count());
console.log("hint:", await back.locator(".nt-model-chain__hint").textContent());
console.log(
  "reset enabled before move:",
  await card.locator('.nt-entitlement-group-card__action[aria-label="重置优先级"]').isEnabled(),
);
console.log("save enabled before move:", await page.getByRole("button", { name: "保存修订" }).isEnabled());

const readChain = async () => {
  const rows = back.locator("[data-model-chain-provider]");
  const count = await rows.count();
  const out = [];
  for (let index = 0; index < count; index += 1) {
    const row = rows.nth(index);
    out.push(
      `${await row.getAttribute("data-model-chain-rank")}:${await row.getAttribute("data-model-chain-provider")}`,
    );
  }
  return out.join(" > ");
};

console.log("chain before:", await readChain());
// Move the second link up, i.e. promote a fallback to primary.
const second = back.locator("[data-model-chain-provider]").nth(1);
await second.locator(".nt-model-chain__moves button").first().click();
await page.waitForTimeout(700);
console.log("chain after promote:", await readChain());
console.log(
  "reset enabled after promote:",
  await card.locator('.nt-entitlement-group-card__action[aria-label="重置优先级"]').isEnabled(),
);
console.log("save enabled after promote:", await page.getByRole("button", { name: "保存修订" }).isEnabled());
console.log(
  "back overflow after promote:",
  await back.evaluate((node) => ({ scroll: node.scrollHeight, client: node.clientHeight })),
);
console.log("hint after promote:", await back.locator(".nt-model-chain__hint").textContent());

// Front should now advertise the promoted provider as primary.
await back.locator(".nt-entitlement-group-card__flip").click();
await page.waitForTimeout(700);
console.log(
  "front primary after promote:",
  await card.locator('[data-model-pool-metric="primary"]').textContent(),
);
await page.screenshot({ path: "output/model-pool-reorder.png", fullPage: false });

// Reset drops the pinned route, so the chain falls back to the inherited order.
await card.locator('.nt-entitlement-group-card__action[aria-label="重置优先级"]').click();
await page.waitForTimeout(700);
await card.locator(".nt-entitlement-group-card__front .nt-entitlement-group-card__flip").click();
await page.waitForTimeout(700);
console.log("chain after reset:", await readChain());
console.log("hint after reset:", await back.locator(".nt-model-chain__hint").textContent());
console.log(
  "reset enabled after reset:",
  await card.locator('.nt-entitlement-group-card__action[aria-label="重置优先级"]').isEnabled(),
);

// The chain pages instead of scrolling once it passes four links.
const paged = await page.evaluate(() => {
  const node = document.querySelector("[data-model-pool-card] .nt-entitlement-group-card__back");
  return node ? { scroll: node.scrollHeight, client: node.clientHeight } : null;
});
console.log("first card back overflow:", JSON.stringify(paged));

await browser.close();
