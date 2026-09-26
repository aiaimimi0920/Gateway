import { normalizeString, previewText } from "./input-text.mjs";

async function bestEffortContinueIntoApp(page, capture) {
  const clickIfVisible = async (locator, label) => {
    try {
      const candidate = locator.first();
      if (await candidate.isVisible({ timeout: 1200 }).catch(() => false)) {
        await candidate.click({ timeout: 5000, force: true });
        capture.autoActions.push({ action: label, ok: true });
        await page.waitForTimeout(1200);
        return true;
      }
    } catch (error) {
      capture.autoActions.push({ action: label, ok: false, error: String(error) });
    }
    return false;
  };

  try {
    const termsCheckbox = page.locator(
      'input[aria-label*="Google API 服务条款"], input[aria-label*="Gemini API"], input[aria-label*="I agree"]',
    );
    const checkbox = termsCheckbox.first();
    if (await checkbox.isVisible({ timeout: 1000 }).catch(() => false)) {
      await checkbox.check({ force: true });
      capture.autoActions.push({ action: "accept-terms-checkbox", ok: true });
      await page.waitForTimeout(400);
    }
  } catch (error) {
    capture.autoActions.push({
      action: "accept-terms-checkbox",
      ok: false,
      error: String(error),
    });
  }

  await clickIfVisible(page.getByRole("button", { name: /Continue to the app/i }), "continue-to-app");
  await clickIfVisible(page.getByRole("button", { name: /^继续$/ }), "continue-cn");
  await clickIfVisible(page.locator("text=Continue to the app"), "continue-to-app-text");
  await clickIfVisible(page.locator("text=继续"), "continue-cn-text");
  await clickIfVisible(page.getByRole("button", { name: /Dismiss/i }), "dismiss-update-banner");
  await clickIfVisible(page.locator(".cdk-overlay-backdrop"), "click-overlay-backdrop");
}

async function bestEffortApplyRemixModal(page, capture) {
  try {
    const remixHeader = page.getByText(/^Remix\b/).first();
    if (!(await remixHeader.isVisible({ timeout: 1200 }).catch(() => false))) {
      return false;
    }
    const applyButton = page.getByRole("button", { name: /^Apply$/i }).first();
    if (!(await applyButton.isVisible({ timeout: 1200 }).catch(() => false))) {
      return false;
    }
    await applyButton.click({ timeout: 5000, force: true });
    capture.autoActions.push({ action: "apply-remix-modal", ok: true });
    await page.waitForTimeout(2000);
    return true;
  } catch (error) {
    capture.autoActions.push({
      action: "apply-remix-modal",
      ok: false,
      error: String(error),
    });
    return false;
  }
}

async function bestEffortDismissAistudioOverlays(page, capture) {
  const clickIfVisible = async (locator, label) => {
    try {
      const candidate = locator.first();
      if (!(await candidate.isVisible({ timeout: 900 }).catch(() => false))) {
        return false;
      }
      await candidate.click({ timeout: 3000, force: true });
      capture.autoActions.push({ action: label, ok: true });
      await page.waitForTimeout(500);
      return true;
    } catch (error) {
      capture.autoActions.push({ action: label, ok: false, error: String(error) });
      return false;
    }
  };

  let dismissed = false;
  await page.keyboard.press("Escape").catch(() => undefined);
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(OK,\s*got it|Got it|Dismiss)$/i }),
      "dismiss-cookie-ok",
    )) || dismissed;
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(Skip|跳过|暂不|Not now)$/i }),
      "dismiss-onboarding-skip",
    )) || dismissed;
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(Continue to the app|Continue|继续)$/i }),
      "dismiss-continue",
    )) || dismissed;
  dismissed =
    (await clickIfVisible(
      page.getByRole("button", { name: /^(Close|关闭)$/i }),
      "dismiss-close",
    )) || dismissed;
  return dismissed;
}

async function bestEffortSelectGoogleAccount(page, capture, accountEmail) {
  const email = normalizeString(accountEmail);
  if (!email) {
    return false;
  }

  let currentUrl = "";
  try {
    currentUrl = String(page.url?.() ?? "");
  } catch (_) {
    currentUrl = "";
  }
  try {
    const parsed = new URL(currentUrl);
    const host = parsed.hostname.toLowerCase();
    if (host !== "accounts.google.com" && !host.endsWith(".accounts.google.com")) {
      return false;
    }
  } catch (_) {
    return false;
  }

  try {
    const accountRow = page.getByText(email, { exact: true }).first();
    if (!(await accountRow.isVisible({ timeout: 1500 }).catch(() => false))) {
      capture.autoActions.push({
        action: "select-google-account",
        ok: false,
        email,
        reason: "account-row-not-visible",
      });
      return false;
    }
    await accountRow.click({ timeout: 5000, force: true });
    capture.autoActions.push({
      action: "select-google-account",
      ok: true,
      email,
    });
    await page.waitForTimeout(3000);
    return true;
  } catch (error) {
    capture.autoActions.push({
      action: "select-google-account",
      ok: false,
      email,
      error: String(error),
    });
    return false;
  }
}

async function bestEffortAutoPrompt(page, promptText, capture, options = {}) {
  const normalized = normalizeString(promptText);
  if (!normalized) {
    return false;
  }

  const maxAttempts = Math.max(1, Number(options.maxAttempts) || 4);
  const pollMs = Math.max(0, Number(options.pollMs) || 1000);

  for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
    const candidates = [
      page.locator('textarea[aria-label="Enter a prompt to generate an app"]').first(),
      page.locator('textarea[placeholder*="Describe an app"]').first(),
      page.locator('textarea[placeholder*="Make changes"]').first(),
      page.locator("textarea").first(),
    ];

    for (const locator of candidates) {
      try {
        if (!(await locator.isVisible({ timeout: 1200 }).catch(() => false))) {
          continue;
        }
        await page.keyboard.press("Escape").catch(() => undefined);
        await page.waitForTimeout(200);
        await locator.click({ timeout: 3000, force: true });
        await locator.fill(normalized, { timeout: 5000, force: true });
        await page.waitForTimeout(300);
        let submission = "ctrl-enter";
        const buildByRole = page.getByRole("button", { name: /^Build/i }).first();
        const buildByText = page.locator("button:has-text('Build')").first();
        if (await buildByRole.isVisible({ timeout: 1200 }).catch(() => false)) {
          await buildByRole.click({ timeout: 5000, force: true });
          submission = "build-button-role";
        } else if (await buildByText.isVisible({ timeout: 1200 }).catch(() => false)) {
          await buildByText.click({ timeout: 5000, force: true });
          submission = "build-button-text";
        } else {
          await page.keyboard.press(
            process.platform === "darwin" ? "Meta+Enter" : "Control+Enter",
          );
        }
        capture.autoActions.push({
          action: "auto-prompt",
          ok: true,
          promptPreview: previewText(normalized, 256),
          submission,
          attempts: attempt,
        });
        return true;
      } catch (error) {
        capture.autoActions.push({
          action: "auto-prompt",
          ok: false,
          attempts: attempt,
          error: String(error),
        });
      }
    }

    if (attempt < maxAttempts && pollMs > 0) {
      await page.waitForTimeout(pollMs);
    }
  }

  capture.autoActions.push({
    action: "auto-prompt",
    ok: false,
    reason: "prompt-textarea-not-visible",
    attempts: maxAttempts,
  });
  return false;
}

function shouldRetryAutoPromptDuringPolling({
  normalizedAutoPrompt,
  autoPromptSubmitted,
  nowMs,
  lastAutoPromptAttemptAtMs,
  retryIntervalMs = 5_000,
}) {
  return (
    Boolean(normalizeString(normalizedAutoPrompt)) &&
    !autoPromptSubmitted &&
    Number(nowMs) - Number(lastAutoPromptAttemptAtMs) >=
      Math.max(0, Number(retryIntervalMs) || 0)
  );
}

async function bestEffortLaunchOwnedApp(page, capture) {
  const clickIfVisible = async (locator, label) => {
    try {
      const candidate = locator.first();
      if (await candidate.isVisible({ timeout: 1200 }).catch(() => false)) {
        await candidate.click({ timeout: 5000, force: true });
        capture.autoActions.push({ action: label, ok: true });
        await page.waitForTimeout(3000);
        return true;
      }
    } catch (error) {
      capture.autoActions.push({ action: label, ok: false, error: String(error) });
    }
    return false;
  };

  if (
    await clickIfVisible(page.getByRole("button", { name: /^Launch/i }), "launch-owned-app-role")
  ) {
    return true;
  }
  if (await clickIfVisible(page.locator("button:has-text('Launch')"), "launch-owned-app-text")) {
    return true;
  }
  return false;
}

export { bestEffortContinueIntoApp, bestEffortApplyRemixModal, bestEffortDismissAistudioOverlays, bestEffortSelectGoogleAccount, bestEffortAutoPrompt, shouldRetryAutoPromptDuringPolling, bestEffortLaunchOwnedApp };
