import { chromium } from "playwright";

const baseUrl = process.env.PROBE_BASE_URL ?? "http://localhost:4200/ui/";
const token = process.env.PROBE_TOKEN ?? "123456";

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
await page.goto(baseUrl, { waitUntil: "domcontentloaded" });

await page.getByLabel("管理密钥").fill(token);
await page.getByRole("button", { name: "登录" }).click();
await page.getByRole("button", { name: "权益组", exact: true }).click();
await page.waitForTimeout(1200);

const card = page.locator(".nt-entitlement-group-card").first();
const cardCount = await page.locator(".nt-entitlement-group-card").count();
console.log("group cards:", cardCount);

if (cardCount > 0) {
  const toggle = card.locator(
    ".nt-entitlement-group-card__front .nt-entitlement-group-card__head-actions .nt-entitlement-group-card__library-toggle",
  );
  console.log("front top-right toggle:", await toggle.count());
  console.log("footer actions:", await card.locator(".nt-entitlement-group-card__front .nt-entitlement-group-card__action--icon").count());
  await toggle.click();
  await page.waitForTimeout(900);
  const panel = page.locator(".nt-entitlement-group-card__accounts--attached");
  console.log("panel visible:", await panel.isVisible());
  console.log("reused pool cards:", await panel.locator(".nt-provider-account-card").count());
  console.log("fallback cards:", await panel.locator(".nt-entitlement-account-card").count());
  await page.screenshot({ path: "output/entitlement-accounts.png", fullPage: false });
}

await browser.close();
