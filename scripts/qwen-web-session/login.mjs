export async function loginExistingAccount(page, baseUrl, authSeed, timeoutMs, createWorkerError) {
  if (!authSeed?.email || !authSeed?.password) {
    throw createWorkerError(
      401,
      "qwen_web_browser_login_seed_missing",
      "Browser-backed Qwen Web login requires an existing account email and plain password.",
    );
  }

  await page.goto(`${baseUrl}/auth`, {
    waitUntil: "domcontentloaded",
    timeout: Math.min(timeoutMs, 60_000),
  });
  await page.waitForTimeout(2_000);

  let emailInput = await page.$('input[placeholder*=\"Email\"]');
  let passwordInput = await page.$('input[type=\"password\"]');
  if (!emailInput || !passwordInput) {
    const inputs = await page.$$('input');
    emailInput = emailInput ?? inputs[0] ?? null;
    if (!passwordInput && inputs.length >= 2) {
      passwordInput = inputs[1];
    }
  }

  if (!emailInput || !passwordInput) {
    throw createWorkerError(
      500,
      "qwen_web_browser_login_form_missing",
      "Could not locate the Qwen Web login form while refreshing an existing account session.",
    );
  }

  await emailInput.click();
  await emailInput.fill(authSeed.email);
  await passwordInput.click();
  await passwordInput.fill(authSeed.password);

  const submitButton =
    (await page.$('button:has-text(\"Log in\")')) ??
    (await page.$('button:has-text(\"登录\")')) ??
    (await page.$('button[type=\"submit\"]'));
  if (!submitButton) {
    throw createWorkerError(
      500,
      "qwen_web_browser_login_submit_missing",
      "Could not locate the Qwen Web login submit button while refreshing an existing account session.",
    );
  }

  await submitButton.click();
  await page.waitForTimeout(8_000);
}
