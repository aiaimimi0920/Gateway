import { chromium } from "playwright";

const baseUrl = process.env.PROBE_BASE_URL ?? "http://localhost:4200/ui/";
const token = process.env.PROBE_TOKEN ?? "123456";

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1600, height: 1000 } });
await page.goto(baseUrl, { waitUntil: "domcontentloaded" });

await page.getByLabel("管理密钥").fill(token);
await page.getByRole("button", { name: "登录" }).click();
await page.getByRole("button", { name: "权益组", exact: true }).click();
await page.waitForTimeout(1500);

const card = page.locator(".nt-entitlement-group-card").first();
console.log("group cards:", await page.locator(".nt-entitlement-group-card").count());
const front = card.locator(".nt-entitlement-group-card__front");
// The front carries two dl bands under the same class: the structural counters
// (providers / accounts / billing) and the usage row, so scope each separately.
console.log(
  "front structural cells:",
  await front
    .locator(
      ".nt-entitlement-group-card__metrics:not(.nt-entitlement-group-card__usage) > div",
    )
    .count(),
);
console.log(
  "front usage cells:",
  await front.locator(".nt-entitlement-group-card__usage > div").count(),
);
for (const metric of ["accounts", "concurrency", "upstream-cost", "platform-revenue", "requests", "success-rate"]) {
  console.log(`  front ${metric}:`, await front.locator(`[data-entitlement-group-metric="${metric}"]`).textContent());
}
console.log("front says 账号数:", await front.getByText("账号数", { exact: true }).count());
console.log("front says 启用 badge:", await front.locator(".nt-badge").count());
console.log("front provider chips:", await front.locator(".nt-chip").count());
console.log("front description paragraphs:", await front.locator("p").count());
console.log("front box:", (await front.boundingBox())?.height);
console.log(
  "front overflow:",
  await front.evaluate((node) => ({ scroll: node.scrollHeight, client: node.clientHeight })),
);

await card.locator(".nt-entitlement-group-card__front .nt-entitlement-group-card__flip").click();
await page.waitForTimeout(700);

const back = card.locator(".nt-entitlement-group-card__back");
console.log("provider tabs:", await back.locator("[data-entitlement-scope-provider]").count());
console.log(
  "pressed tabs:",
  await back.locator('[data-entitlement-scope-provider][aria-pressed="true"]').count(),
);
console.log("model rows:", await back.locator("[data-entitlement-scope-model]").count());
console.log("pagers:", await back.locator(".nt-entitlement-scope__pager").count());
const firstRow = back.locator("[data-entitlement-scope-model]").first();
if ((await firstRow.count()) > 0) {
  console.log(
    "row model:",
    await firstRow.locator(".nt-entitlement-scope__model-head strong").textContent(),
  );
  for (const metric of ["accounts", "concurrency", "upstream-cost", "platform-revenue", "requests", "success-rate"]) {
    console.log(
      `  ${metric}:`,
      await firstRow.locator(`[data-entitlement-model-metric="${metric}"]`).textContent(),
    );
  }
  console.log(
    "  head has right-aligned concurrency:",
    await firstRow.locator(".nt-entitlement-scope__model-head .nt-entitlement-scope__model-concurrency").count(),
  );
  console.log(
    "  metric dl cells:",
    await firstRow.locator(".nt-entitlement-scope__model-metrics > div").count(),
  );
  console.log(
    "  availability cells:",
    await firstRow.locator(".nt-entitlement-scope__model-success .nt-provider-card__availability-cell").count(),
  );
  console.log("  row height:", (await firstRow.boundingBox())?.height);
}

const box = await back.boundingBox();
const inner = await back.locator(".nt-entitlement-scope").boundingBox();
console.log("back box:", box?.height, "scope content:", inner?.height);
console.log(
  "back overflow:",
  await back.evaluate((node) => ({ scroll: node.scrollHeight, client: node.clientHeight })),
);
console.log(
  "scope overflow:",
  await back
    .locator(".nt-entitlement-scope")
    .evaluate((node) => ({ scroll: node.scrollHeight, client: node.clientHeight })),
);

await page.screenshot({ path: "output/entitlement-scope-back.png", fullPage: false });

// Expand the account panel, then narrow it from the back's provider tabs.
await card.locator(".nt-entitlement-group-card__back .nt-entitlement-group-card__library-toggle").click();
await page.waitForTimeout(900);
const panel = page.locator(".nt-entitlement-group-card__accounts--attached").first();
console.log(
  "panel count badge:",
  await panel.locator(".nt-entitlement-group-card__accounts-count").textContent(),
);
console.log("panel account cards:", await panel.locator(".nt-provider-account-card").count());

const firstTab = back.locator("[data-entitlement-scope-provider]").first();
await firstTab.click();
await page.waitForTimeout(600);
console.log("after deselect -> model rows:", await back.locator("[data-entitlement-scope-model]").count());
console.log(
  "after deselect -> pressed tabs:",
  await back.locator('[data-entitlement-scope-provider][aria-pressed="true"]').count(),
);
console.log(
  "after deselect -> panel count badge:",
  await panel.locator(".nt-entitlement-group-card__accounts-count").textContent(),
);
console.log(
  "after deselect -> panel account cards:",
  await panel.locator(".nt-provider-account-card").count(),
);
console.log(
  "after deselect -> panel scope hint:",
  await panel.locator(".nt-entitlement-group-card__accounts-scope").count(),
);
await page.screenshot({ path: "output/entitlement-scope-back-narrowed.png", fullPage: false });

await browser.close();
