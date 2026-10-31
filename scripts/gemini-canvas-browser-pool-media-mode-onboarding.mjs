// Only acknowledge the known informational video dialog, never a purchase or trial.
export async function dismissMediaModeOnboarding(page, operation, timeoutMs) {
  if (operation !== "video") return true;
  try {
    // Route readiness can precede the asynchronously mounted introduction modal.
    try {
      await page.getByRole("button", { name: /^(?:立即体验|Try (?:it )?now)$/i }).first()
        .waitFor({ state: "visible", timeout: Math.min(timeoutMs, 5000) });
    } catch { /* Absence is normal for accounts that already acknowledged it. */ }
    const dialog = page.getByRole("dialog", { name: /.*/ }).first();
    if (!await dialog.count()) return true;
    const acknowledge = dialog.getByRole("button", { name: /^(?:立即体验|Try (?:it )?now)$/i }).first();
    // Angular can leave the container at zero bounds while its child button is visible.
    if (!await dialog.isVisible() && !await acknowledge.isVisible()) return true;
    const text = await dialog.innerText({ timeout: Math.min(timeoutMs, 2000) });
    if (text.length > 4096 || !/^(?:创作视频|Create videos?)\s/i.test(text)
      || !/Gemini Omni/i.test(text)
      || /收费|订阅|付款|升级|免费试用|subscribe|subscription|billing|payment|upgrade|free trial|[$€£￥]/i.test(text)) return false;
    if (!await acknowledge.isVisible()) return false;
    await acknowledge.click({ timeout: Math.min(timeoutMs, 5000) });
    await acknowledge.waitFor({ state: "hidden", timeout: Math.min(timeoutMs, 5000) });
    return true;
  } catch { return false; }
}
