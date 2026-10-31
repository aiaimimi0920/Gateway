// Small viewports move media tools behind a second, explicitly labelled menu.
export async function revealAdditionalOperationTools(page, config, timeoutMs) {
  for (const role of ["button", "link", "menuitem", "menuitemcheckbox"]) {
    if (await page.getByRole(role, { name: config.buttonName }).first().isVisible()) return;
  }
  const more = page.getByRole("button", { name: /^(?:更多工具|More tools)$/i }).first();
  if (!await more.isVisible()) return;
  // This only opens a menu; prompt submission remains owned by the single-submit path.
  await more.click({ timeout: Math.min(timeoutMs, 5000) });
  await page.waitForTimeout(300);
}
