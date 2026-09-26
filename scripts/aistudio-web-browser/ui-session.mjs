import { normalizeString } from "./settings.mjs";

export async function clickIfVisible(page, locator, label) {
  try {
    const candidate = locator.first();
    if (await candidate.isVisible({ timeout: 1200 }).catch(() => false)) {
      await candidate.click({ timeout: 5000, force: true });
      await page.waitForTimeout(1200);
      return { ok: true, action: label };
    }
  } catch (error) {
    return { ok: false, action: label, error: String(error) };
  }
  return { ok: false, action: label, skipped: true };
}

export async function bestEffortSelectGoogleAccount(page) {
  const directCandidates = [
    page.locator('button[name="chooser[select]"]').first(),
    page.locator('li button[name="chooser[select]"]').first(),
  ];
  for (const locator of directCandidates) {
    try {
      if (await locator.isVisible({ timeout: 1200 }).catch(() => false)) {
        await locator.click({ timeout: 5000, force: true });
        await page.waitForURL(/ai\.studio|aistudio\.google\.com/i, {
          timeout: 20_000,
        }).catch(() => undefined);
        await page.waitForTimeout(1500);
        return { ok: true, action: "select-google-account:chooser-select-button" };
      }
    } catch (_) {}
  }

  const selectors = [
    '[data-identifier]',
    '[data-email]',
    '[role="link"]',
    'li',
    'div[jsaction]',
  ];
  for (const selector of selectors) {
    try {
      const clicked = await page.evaluate((candidateSelector) => {
        const isVisible = (node) => {
          if (!(node instanceof HTMLElement)) {
            return false;
          }
          const style = window.getComputedStyle(node);
          const rect = node.getBoundingClientRect();
          return (
            style.visibility !== "hidden" &&
            style.display !== "none" &&
            rect.width >= 24 &&
            rect.height >= 18
          );
        };

        const toClickable = (node) => {
          if (!(node instanceof HTMLElement)) {
            return null;
          }
          let current = node;
          for (let depth = 0; depth < 6 && current; depth += 1) {
            const role = current.getAttribute("role") || "";
            if (
              current.tagName === "A" ||
              current.tagName === "BUTTON" ||
              current.hasAttribute("jsaction") ||
              current.hasAttribute("data-identifier") ||
              current.hasAttribute("data-email") ||
              role === "link" ||
              role === "button" ||
              current.tabIndex >= 0
            ) {
              return current;
            }
            current = current.parentElement;
          }
          return node;
        };

        const candidates = Array.from(document.querySelectorAll(candidateSelector))
          .filter((node) => node instanceof HTMLElement)
          .filter((node) => isVisible(node))
          .map((node) => ({
            node,
            text: (node.textContent || "").trim(),
          }))
          .filter(
            (entry) =>
              entry.text.includes("@") &&
              !/Use another account|Remove an account/i.test(entry.text),
          );

        if (!candidates.length) {
          return false;
        }
        const target = toClickable(candidates[0].node);
        if (!(target instanceof HTMLElement)) {
          return false;
        }
        target.click();
        return true;
      }, selector);
      if (clicked) {
        await page.waitForURL(/ai\.studio|aistudio\.google\.com/i, {
          timeout: 20_000,
        }).catch(() => undefined);
        await page.waitForTimeout(1500);
        return { ok: true, action: `select-google-account:${selector}` };
      }
    } catch (_) {}
  }
  return { ok: false, action: "select-google-account", skipped: true };
}

export async function bestEffortContinueIntoApp(page) {
  if (page.url().includes("accounts.google.com")) {
    const accountClicked = await bestEffortSelectGoogleAccount(page);
    if (accountClicked.ok) {
      return;
    }
  }
  try {
    const termsCheckbox = page.locator(
      'input[aria-label*="Google API 服务条款"], input[aria-label*="Gemini API"], input[aria-label*="I agree"]',
    );
    const checkbox = termsCheckbox.first();
    if (await checkbox.isVisible({ timeout: 1000 }).catch(() => false)) {
      await checkbox.check({ force: true });
      await page.waitForTimeout(400);
    }
  } catch (_) {}

  await clickIfVisible(page, page.getByRole("button", { name: /Continue to the app/i }), "continue-to-app");
  await clickIfVisible(page, page.getByRole("button", { name: /^继续$/ }), "continue-cn");
  await clickIfVisible(page, page.locator("text=Continue to the app"), "continue-to-app-text");
  await clickIfVisible(page, page.locator("text=继续"), "continue-cn-text");
  await clickIfVisible(page, page.getByRole("button", { name: /Dismiss/i }), "dismiss-update-banner");
  await clickIfVisible(page, page.getByRole("button", { name: /Back to start/i }), "back-to-start-role");
  await clickIfVisible(page, page.locator("button:has-text('Back to start')"), "back-to-start-text");
  for (let step = 0; step < 4; step += 1) {
    const nextByRole = page.getByRole("button", { name: /^Next$/i }).first();
    const nextByText = page.locator("button:has-text('Next')").first();
    const doneByRole = page.getByRole("button", { name: /Done|Got it|Skip/i }).first();
    const closeByLabel = page.locator('[aria-label="Close"], [aria-label="Dismiss"]').first();
    const clicked =
      (await clickIfVisible(page, nextByRole, `intro-next-role-${step}`)).ok ||
      (await clickIfVisible(page, nextByText, `intro-next-text-${step}`)).ok ||
      (await clickIfVisible(page, doneByRole, `intro-finish-${step}`)).ok ||
      (await clickIfVisible(page, closeByLabel, `intro-close-${step}`)).ok;
    if (!clicked) {
      break;
    }
    await page.waitForTimeout(250);
  }
  await clickIfVisible(page, page.locator(".cdk-overlay-backdrop"), "click-overlay-backdrop");
  await page.keyboard.press("Escape").catch(() => undefined);
}

export async function ensureAuthenticatedStudioPage(page, timeoutMs = 20_000) {
  const deadline = Date.now() + Math.max(timeoutMs, 5_000);
  while (Date.now() < deadline) {
    const currentUrl = page.url();
    if (currentUrl.includes("ai.studio") || currentUrl.includes("aistudio.google.com")) {
      return true;
    }
    await bestEffortContinueIntoApp(page);
    if (page.url().includes("accounts.google.com")) {
      await bestEffortSelectGoogleAccount(page);
    }
    await page.waitForTimeout(800);
  }
  const finalUrl = page.url();
  return finalUrl.includes("ai.studio") || finalUrl.includes("aistudio.google.com");
}

export async function bestEffortLaunchOwnedApp(page) {
  if (
    (await clickIfVisible(page, page.getByRole("button", { name: /^Launch/i }), "launch-owned-app-role"))
      .ok
  ) {
    return true;
  }
  if ((await clickIfVisible(page, page.locator("button:has-text('Launch')"), "launch-owned-app-text")).ok) {
    return true;
  }
  return false;
}

export async function sendActiveTrigger(page) {
  try {
    await page.evaluate(async () => {
      try {
        await fetch("https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger", {
          method: "GET",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
        });
      } catch (_) {}
    });
  } catch (_) {}
}

export async function probeRunAppFrameFetch(page, requestSpec) {
  const runAppFrame = page.frames().find((frame) => frame.url().includes("run.app"));
  if (!runAppFrame) {
    return null;
  }
  try {
    await runAppFrame.evaluate(async (spec) => {
      try {
        await fetch(spec.url, {
          method: spec.method || "GET",
          headers: spec.headers || {},
          body: typeof spec.body === "string" ? spec.body : undefined,
          credentials: spec.credentials || "omit",
        });
      } catch (_) {}
    }, requestSpec);
  } catch (_) {}
  return true;
}

export async function warmupPromptSurface(page, localWebSocketServer = null) {
  await ensureAuthenticatedStudioPage(page, 25_000).catch(() => false);
  await bestEffortContinueIntoApp(page);
  await bestEffortLaunchOwnedApp(page);
  await page.waitForTimeout(1200);
  await sendActiveTrigger(page);
  await page.waitForTimeout(800);
  await probeRunAppFrameFetch(page, {
    method: "GET",
    url: "https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger",
    headers: {
      "content-type": "application/json",
    },
  });
  await localWebSocketServer?.waitForConnection(12_000).catch(() => false);
  await page.waitForTimeout(1000);
}

export async function bestEffortAutoPrompt(page, promptText, options = {}) {
  const normalized = normalizeString(promptText);
  if (!normalized) {
    return false;
  }
  const preferKeyboardSubmit = options?.preferKeyboardSubmit === true;
  const localWebSocketServer = options?.localWebSocketServer ?? null;

  const candidates = [
    page.locator('textarea[aria-label="Enter a prompt to generate an app"]').first(),
    page.locator('textarea[placeholder*="Describe an app"]').first(),
    page.locator('textarea[placeholder*="Make changes"]').first(),
    page.getByRole("textbox", { name: /prompt|generate|app|change/i }).first(),
    page.locator('[contenteditable="true"][role="textbox"]').first(),
    page.locator('[contenteditable="true"][aria-label*="prompt" i]').first(),
    page.locator('[contenteditable="true"]').first(),
    page.locator("textarea").first(),
  ];

  const startedAt = Date.now();
  let lastRecoveryAt = 0;
  while (Date.now() - startedAt < 30_000) {
    if (Date.now() - lastRecoveryAt >= 4_000) {
      lastRecoveryAt = Date.now();
      await warmupPromptSurface(page, localWebSocketServer).catch(() => undefined);
      await page.keyboard.press("Escape").catch(() => undefined);
      await page.waitForTimeout(150);
    }
    for (const locator of candidates) {
      try {
        if (!(await locator.isVisible({ timeout: 1200 }).catch(() => false))) {
          continue;
        }
        await page.keyboard.press("Escape").catch(() => undefined);
        await page.waitForTimeout(200);
        await locator.click({ timeout: 3000, force: true });
        try {
          await locator.fill(normalized, { timeout: 5000, force: true });
        } catch (_) {
          await locator.evaluate((node, value) => {
            const element = /** @type {HTMLTextAreaElement | HTMLInputElement | HTMLElement} */ (node);
            if ("value" in element) {
              element.value = value;
            } else {
              element.textContent = value;
            }
            element.dispatchEvent(new Event("input", { bubbles: true }));
            element.dispatchEvent(new Event("change", { bubbles: true }));
          }, normalized);
        }
        await page.waitForTimeout(300);
        if (preferKeyboardSubmit) {
          await page.keyboard.press(process.platform === "darwin" ? "Meta+Enter" : "Control+Enter");
        } else {
          const buildByRole = page.getByRole("button", { name: /^Build/i }).first();
          const buildByText = page.locator("button:has-text('Build')").first();
          if (await buildByRole.isVisible({ timeout: 1200 }).catch(() => false)) {
            await buildByRole.click({ timeout: 5000, force: true });
          } else if (await buildByText.isVisible({ timeout: 1200 }).catch(() => false)) {
            await buildByText.click({ timeout: 5000, force: true });
          } else {
            await page.keyboard.press(process.platform === "darwin" ? "Meta+Enter" : "Control+Enter");
          }
        }
        return true;
      } catch (_) {}
    }
    await page.waitForTimeout(500);
  }

  return false;
}
