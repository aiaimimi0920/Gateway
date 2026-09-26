import { safePageUrl } from "./page-state.mjs";

export async function navigateWithChallengeSettle(page, url, timeoutMs) {
  await page.goto(url, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  });
  await page.waitForTimeout(2_500);

  for (let index = 0; index < 2; index += 1) {
    const html = await page.content().catch(() => "");
    if (!detectBrowserChallenge(html) && !detectBrowserChallenge(safePageUrl(page))) {
      return;
    }
    await page.waitForTimeout(4_000);
    await page.reload({
      waitUntil: "domcontentloaded",
      timeout: Math.min(timeoutMs, 60_000),
    }).catch(() => {});
  }
}

export function detectBrowserChallenge(value) {
  const lower = String(value ?? "").toLowerCase();
  return (
    lower.includes("cloudflare") ||
    lower.includes("captcha") ||
    lower.includes("challenge") ||
    lower.includes("verify you are human") ||
    lower.includes("just a moment") ||
    lower.includes("attention required")
  );
}
