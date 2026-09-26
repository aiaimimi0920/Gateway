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

console.log("model cards:", await page.locator("[data-model-pool-card]").count());
console.log(
  "board header label:",
  await page.locator(".nt-board__title, .nt-shell-board__title").first().textContent().catch(() => null),
);
console.log("save button:", await page.getByRole("button", { name: "保存修订" }).count());

const card = page.locator("[data-model-pool-card]").first();
console.log("first model:", await card.getAttribute("data-model-pool-card"));
const front = card.locator(".nt-entitlement-group-card__front");
console.log(
  "front structural cells:",
  await front
    .locator(".nt-entitlement-group-card__metrics:not(.nt-entitlement-group-card__usage) > div")
    .count(),
);
console.log(
  "front usage cells:",
  await front.locator(".nt-entitlement-group-card__usage > div").count(),
);
for (const metric of [
  "providers",
  "accounts",
  "primary",
  "concurrency",
  "upstream-cost",
  "platform-revenue",
  "requests",
  "success-rate",
]) {
  console.log(`  front ${metric}:`, await front.locator(`[data-model-pool-metric="${metric}"]`).textContent());
}
console.log("front footer actions:", await front.locator(".nt-entitlement-group-card__action").count());
console.log("front box:", (await front.boundingBox())?.height);
console.log(
  "front overflow:",
  await front.evaluate((node) => ({ scroll: node.scrollHeight, client: node.clientHeight })),
);

await front.locator(".nt-entitlement-group-card__flip").click();
await page.waitForTimeout(700);

const back = card.locator(".nt-entitlement-group-card__back");
console.log("chain links:", await back.locator("[data-model-chain-provider]").count());
console.log(
  "selected links:",
  await back.locator('.nt-model-chain__label[aria-pressed="true"]').count(),
);
console.log("pagers:", await back.locator(".nt-entitlement-scope__pager").count());
console.log("hint:", await back.locator(".nt-model-chain__hint").textContent());

const firstLink = back.locator("[data-model-chain-provider]").first();
console.log("link 1 provider:", await firstLink.getAttribute("data-model-chain-provider"));
console.log("link 1 rank:", await firstLink.getAttribute("data-model-chain-rank"));
console.log("link 1 label:", await firstLink.locator(".nt-model-chain__label strong").textContent());
for (const metric of ["concurrency", "accounts", "upstream-cost", "platform-revenue", "requests", "success-rate"]) {
  console.log(`  link 1 ${metric}:`, await firstLink.locator(`[data-model-chain-metric="${metric}"]`).textContent());
}
console.log("  link 1 move buttons:", await firstLink.locator(".nt-model-chain__moves button").count());
console.log(
  "  link 1 up disabled:",
  await firstLink.locator(".nt-model-chain__moves button").first().isDisabled(),
);
console.log("  link 1 height:", (await firstLink.boundingBox())?.height);

const backBox = await back.boundingBox();
const inner = await back.locator(".nt-entitlement-scope").boundingBox();
console.log("back box:", backBox?.height, "scope content:", inner?.height);
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

await page.screenshot({ path: "output/model-pool-back.png", fullPage: false });

// Reorder: move link 1 down, then confirm the ranks swapped and the draft went dirty.
const linkCount = await back.locator("[data-model-chain-provider]").count();
if (linkCount > 1) {
  const before = await back.locator(".nt-model-chain__label strong").allTextContents();
  await firstLink.locator(".nt-model-chain__moves button").nth(1).click();
  await page.waitForTimeout(600);
  console.log("chain before move:", before.join(" > "));
  console.log(
    "chain after move:",
    (await back.locator(".nt-model-chain__label strong").allTextContents()).join(" > "),
  );
  console.log(
    "save enabled after move:",
    await page.getByRole("button", { name: "保存修订" }).isEnabled(),
  );
  console.log(
    "reset enabled after move:",
    await card
      .locator('.nt-entitlement-group-card__action[aria-label="重置优先级"]')
      .isEnabled()
      .catch(() => null),
  );
  console.log(
    "back overflow after move:",
    await back.evaluate((node) => ({ scroll: node.scrollHeight, client: node.clientHeight })),
  );
} else {
  console.log("chain has a single link; skipped the reorder probe");
}

// Expand the account panel, then narrow it from the chain rows.
await back.locator(".nt-entitlement-group-card__library-toggle").click();
await page.waitForTimeout(900);
const panel = page.locator(".nt-entitlement-group-card__accounts--attached").first();
console.log(
  "panel count badge:",
  await panel.locator(".nt-entitlement-group-card__accounts-count").textContent(),
);
console.log("panel account cards:", await panel.locator(".nt-provider-account-card").count());

if (linkCount > 1) {
  await back.locator(".nt-model-chain__label").first().click();
  await page.waitForTimeout(600);
  console.log(
    "after deselect -> selected links:",
    await back.locator('.nt-model-chain__label[aria-pressed="true"]').count(),
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
}
await page.screenshot({ path: "output/model-pool-back-narrowed.png", fullPage: false });

// Every card measures the same, front and back, or the grid would ripple.
const heights = await page.evaluate(() =>
  [...document.querySelectorAll("[data-model-pool-card]")].map((node) => {
    const front = node.querySelector(".nt-entitlement-group-card__front");
    const back = node.querySelector(".nt-entitlement-group-card__back");
    return {
      model: node.getAttribute("data-model-pool-card"),
      front: front ? front.getBoundingClientRect().height : null,
      back: back ? back.getBoundingClientRect().height : null,
      frontOverflow: front ? front.scrollHeight - front.clientHeight : null,
      backOverflow: back ? back.scrollHeight - back.clientHeight : null,
    };
  }),
);
console.log("card geometry:", JSON.stringify(heights));

await browser.close();
