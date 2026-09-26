import { createWorkerError } from "./errors.mjs";
import { collectPageState, compactPageState, pageStateRequiresEmailVerification, pageStateRequiresAuthRecovery, safePageUrl } from "./page-state.mjs";
import { navigateToEmailSurface, clickPreLoginButtons, clickButtonByExactText } from "./auth-navigation.mjs";
import {
  ensureRecoveredMailboxContext,
  fetchMailboxLatestMarker,
  fetchMailboxSnapshotLatestMarker,
  fetchImmediateMailboxCode,
  waitForMailboxOpenAiCode,
} from "./mailbox-poll.mjs";

export async function loginExistingAccount(
  page,
  baseUrl,
  authUrl,
  authSeed,
  timeoutMs,
  setStage = () => {},
  mailboxContext = null,
) {
  if (!authSeed?.email || !authSeed?.password) {
    throw createWorkerError(
      401,
      "chatgpt_web_browser_login_seed_missing",
      "Browser-backed ChatGPT Web login requires an existing account email and plain password.",
    );
  }

  const emailSelectors = 'input[type="email"], input[name="email"], input[autocomplete="username"]';
  setStage("login_navigate_email_surface");
  const emailPageState = await navigateToEmailSurface(page, {
    baseUrl,
    authUrl,
    timeoutMs,
  });

  let emailInput = page.locator(emailSelectors).first();
  for (let attempt = 0; attempt < 8; attempt += 1) {
    if ((await emailInput.count()) > 0) {
      break;
    }
    await clickPreLoginButtons(page).catch(() => {});
    await page.waitForTimeout(1_000);
    emailInput = page.locator(emailSelectors).first();
  }
  if ((await emailInput.count()) === 0) {
    const domSubmitted = await submitAuthEmailViaDom(page, authSeed.email);
    if (domSubmitted) {
      await page.waitForTimeout(2_000);
    } else {
      throw createWorkerError(
        500,
        "chatgpt_web_browser_login_email_missing",
        `Could not locate the ChatGPT/OpenAI email input while refreshing an existing account session. url=${safePageUrl(page)} state=${compactPageState(emailPageState)}`,
      );
    }
  } else {
    setStage("login_fill_email");
    const filledEmail = await fillVisibleInputBySelectors(page, emailSelectors, authSeed.email);
    if (!filledEmail) {
      const domSubmitted = await submitAuthEmailViaDom(page, authSeed.email);
      if (!domSubmitted) {
        const emailState = await collectPageState(page);
        throw createWorkerError(
          500,
          "chatgpt_web_browser_login_email_fill_failed",
          `ChatGPT/OpenAI login found an email input node but could not interact with it. state=${compactPageState(emailState)}`,
        );
      }
      await page.waitForTimeout(2_000);
    } else {
      setStage("login_submit_email");
      for (let attempt = 0; attempt < 5; attempt += 1) {
        const clicked = await clickButtonByExactText(page, [
          "Continue with email",
          "使用电子邮件继续",
          "Continue",
          "继续",
        ]);
        if (clicked) {
          break;
        }
        await page.keyboard.press("Enter").catch(() => {});
        await page.waitForTimeout(1_000);
      }
      await page.waitForTimeout(2_500);
    }
  }
  let emailVerificationHandled = await maybeCompleteEmailVerification(
    page,
    mailboxContext,
    timeoutMs,
    setStage,
    authSeed.email,
  );

  const passwordInput = page.locator('input[type="password"]').first();
  let lastBodyText = "";
  setStage("login_wait_password_surface");
  const passwordRecoveryTimeoutMs = Math.min(timeoutMs, 3_000);
  for (let attempt = 0; attempt < 2; attempt += 1) {
    const loopState = await collectPageState(page);
    if (pageStateRequiresEmailVerification(loopState) && !emailVerificationHandled) {
      emailVerificationHandled = await maybeCompleteEmailVerification(
        page,
        mailboxContext,
        timeoutMs,
        setStage,
        authSeed.email,
      );
    }
    lastBodyText = await page.locator("body").innerText().catch(() => "");
    if (lastBodyText.includes("Operation timed out") || lastBodyText.includes("重试")) {
      const retryButton = page.locator('button:has-text("重试"), button:has-text("Retry")').first();
      if ((await retryButton.count()) > 0) {
        await retryButton.click().catch(() => {});
      } else {
        await page.reload({
          waitUntil: "domcontentloaded",
          timeout: passwordRecoveryTimeoutMs,
        }).catch(() => {});
      }
    }
    await page.waitForTimeout(1_500);
    if ((await passwordInput.count()) > 0) {
      break;
    }
  }
  if ((await passwordInput.count()) === 0) {
    const passwordState = await collectPageState(page);
    if (pageStateRequiresEmailVerification(passwordState) && !emailVerificationHandled) {
      emailVerificationHandled = await maybeCompleteEmailVerification(
        page,
        mailboxContext,
        timeoutMs,
        setStage,
        authSeed.email,
      );
      await page.waitForTimeout(2_000);
    }
  }
  if ((await passwordInput.count()) === 0) {
    const passwordState = await collectPageState(page);
    const currentHref = String(passwordState?.href ?? "").toLowerCase();
    const normalizedBaseUrl = String(baseUrl ?? "").toLowerCase();
    const homeRecovered =
      normalizedBaseUrl &&
      currentHref.startsWith(normalizedBaseUrl) &&
      !pageStateRequiresAuthRecovery(passwordState) &&
      !passwordState?.hasEmail &&
      !passwordState?.hasPassword;
    if (homeRecovered) {
      return;
    }
    if (pageStateRequiresEmailVerification(passwordState) && emailVerificationHandled) {
      throw createWorkerError(
        500,
        "chatgpt_web_email_otp_submit_stuck",
        `ChatGPT/OpenAI email verification remained on the OTP page after one repo-owned OTP/password fallback attempt. state=${compactPageState(passwordState)}`,
      );
    }
    if (lastBodyText.includes("Operation timed out") || lastBodyText.includes("重试")) {
      throw createWorkerError(
        503,
        "chatgpt_web_browser_login_timed_out",
        `ChatGPT/OpenAI browser login stalled on the auth page with \`Operation timed out\`, so the worker could not reach the password step. state=${compactPageState(passwordState)}`,
      );
    }
    throw createWorkerError(
      500,
      "chatgpt_web_browser_login_password_missing",
      `Could not locate the ChatGPT/OpenAI password input while refreshing an existing account session. state=${compactPageState(passwordState)}`,
    );
  }
  setStage("login_fill_password");
  const filledPassword = await fillVisibleInputBySelectors(
    page,
    'input[type="password"], input[name="password"], input[name="new-password"]',
    authSeed.password,
  );
  if (!filledPassword) {
    const passwordState = await collectPageState(page);
    throw createWorkerError(
      500,
      "chatgpt_web_browser_login_password_fill_failed",
      `ChatGPT/OpenAI password field existed but could not be filled. state=${compactPageState(passwordState)}`,
    );
  }

  const submitButton = page
    .locator(
      'button:has-text("Log in"), button:has-text("登录"), button:has-text("Continue"), button:has-text("继续"), button[type="submit"]',
    )
    .first();
  setStage("login_submit_password");
  const clickedSubmitButton = await clickButtonByExactText(page, [
    "Log in",
    "登录",
    "Continue",
    "继续",
  ]);
  if (clickedSubmitButton) {
    // already clicked above
  } else if ((await submitButton.count()) > 0) {
    await submitButton.click({ timeout: 15_000 }).catch(() => {});
  } else {
    await page.keyboard.press("Enter").catch(() => {});
  }

  setStage("login_post_submit_wait");
  await page.waitForTimeout(8_000);
}

export async function maybeCompleteEmailVerification(
  page,
  mailboxContext,
  timeoutMs,
  setStage = () => {},
  emailAddress = null,
) {
  const verificationStartedAt = Date.now();
  const verificationMarkerFloor = verificationStartedAt - 5_000;
  const verificationState = await collectPageState(page);
  if (!pageStateRequiresEmailVerification(verificationState)) {
    return false;
  }
  setStage("login_email_verification_choose_password");
  const usedPasswordFallback = await clickButtonByExactText(page, [
    "使用密码继续",
    "Continue with password",
    "Use password instead",
  ]);
  if (usedPasswordFallback) {
    await page.waitForTimeout(3_000);
    const passwordFallbackState = await collectPageState(page);
    if (passwordFallbackState?.hasEmail && emailAddress) {
      await submitEmailAddress(page, emailAddress);
      await page.waitForTimeout(2_500);
    }
    return true;
  }
  if (!mailboxContext?.mailboxRef || !mailboxContext?.mailboxSessionId) {
    throw createWorkerError(
      412,
      "chatgpt_web_mailbox_context_missing",
      `ChatGPT/OpenAI login reached email verification, but no mailboxRef/mailboxSessionId was available. state=${compactPageState(verificationState)}`,
    );
  }
  if (!mailboxContext?.serviceBaseUrl || !mailboxContext?.apiKey) {
    throw createWorkerError(
      412,
      "chatgpt_web_mailbox_service_missing",
      `ChatGPT/OpenAI login reached email verification, but mailbox service configuration is missing. state=${compactPageState(verificationState)}`,
    );
  }

  const effectiveMailboxContext = await ensureRecoveredMailboxContext(mailboxContext);
  setStage("login_wait_email_otp");
  let code = await fetchImmediateMailboxCode(effectiveMailboxContext, {
    minMarker: verificationMarkerFloor,
  }).catch(() => "");
  if (!code) {
    const minMarker = await fetchMailboxLatestMarker(effectiveMailboxContext).catch(() =>
      fetchMailboxSnapshotLatestMarker(effectiveMailboxContext).catch(() => 0),
    );
    await clickOtpResendIfAvailable(page);
    code = await waitForMailboxOpenAiCode(effectiveMailboxContext, {
      timeoutMs: Math.min(Math.max(timeoutMs, 15_000), 30_000),
      minMarker: Math.max(Number(minMarker || 0), verificationMarkerFloor),
    });
  }
  setStage("login_fill_email_otp");
  await fillEmailVerificationCode(page, code);
  setStage("login_submit_email_otp");
  const continueButton = page
    .locator('button:has-text("继续"), button:has-text("Continue"), button[type="submit"]')
    .first();
  if ((await continueButton.count()) > 0) {
    await continueButton.click({ timeout: 10_000 }).catch(() => {});
  } else {
    await page.keyboard.press("Enter").catch(() => {});
  }
  await page.waitForTimeout(4_000);
  const postOtpState = await collectPageState(page);
  if (pageStateRequiresEmailVerification(postOtpState)) {
    throw createWorkerError(
      500,
      "chatgpt_web_email_otp_submit_stuck",
      `ChatGPT/OpenAI email verification remained on the OTP page after submitting a recovered code. state=${compactPageState(postOtpState)}`,
    );
  }
  return true;
}

export async function clickOtpResendIfAvailable(page) {
  const retryButton = page
    .locator('button:has-text("重新发送电子邮件"), button:has-text("Resend email"), button:has-text("Resend")')
    .first();
  if ((await retryButton.count()) > 0) {
    await retryButton.click({ timeout: 5_000 }).catch(() => {});
    await page.waitForTimeout(1_500);
    return true;
  }
  return false;
}

export async function fillEmailVerificationCode(page, code) {
  const digits = String(code ?? "").trim();
  if (!/^\d{6}$/.test(digits)) {
    throw createWorkerError(
      500,
      "chatgpt_web_mailbox_code_invalid",
      "Mailbox OTP helper did not produce a valid 6-digit OpenAI verification code.",
    );
  }
  const inputs = page.locator(
    'input[autocomplete="one-time-code"], input[inputmode="numeric"], input[name*="otp" i], input[name*="code" i], input[type="tel"], input[type="number"]',
  );
  const count = await inputs.count().catch(() => 0);
  if (count <= 0) {
    throw createWorkerError(
      500,
      "chatgpt_web_email_verification_input_missing",
      "ChatGPT/OpenAI email verification page did not expose any OTP input fields.",
    );
  }
  if (count === 1) {
    await inputs.first().fill(digits);
    return;
  }
  for (let index = 0; index < Math.min(6, count); index += 1) {
    await inputs.nth(index).fill(digits[index] ?? "");
  }
}

export async function submitEmailAddress(page, emailAddress) {
  const emailSelectors = 'input[type="email"], input[name="email"], input[autocomplete="username"]';
  const emailInput = page.locator(emailSelectors).first();
  if ((await emailInput.count()) <= 0) {
    return false;
  }
  const filled = await fillVisibleInputBySelectors(page, emailSelectors, String(emailAddress ?? ""));
  if (!filled) {
    return false;
  }
  await page.keyboard.press("Tab").catch(() => {});
  if (
    await clickButtonByExactText(page, [
      "Continue with email",
      "使用电子邮件继续",
      "Continue",
      "继续",
    ])
  ) {
    return true;
  }
  await page.keyboard.press("Enter").catch(() => {});
  return true;
}

export async function fillVisibleInputBySelectors(page, selectors, value) {
  const locator = page.locator(selectors);
  const count = await locator.count().catch(() => 0);
  for (let index = 0; index < count; index += 1) {
    const input = locator.nth(index);
    const visible = await input.isVisible().catch(() => false);
    const enabled = await input.isEnabled().catch(() => false);
    if (!visible || !enabled) {
      continue;
    }
    const filled = await input
      .click({ timeout: 5_000 })
      .then(async () => {
        await input.fill("");
        await input.pressSequentially(String(value ?? ""), { delay: 40 });
        return true;
      })
      .catch(() => false);
    if (filled) {
      return true;
    }
  }

  return page
    .evaluate(
      ({ selectors, value }) => {
        const candidates = Array.from(document.querySelectorAll(selectors));
        const visibleCandidate =
          candidates.find((element) => {
            const style = window.getComputedStyle(element);
            const rect = element.getBoundingClientRect();
            return (
              rect.width > 0 &&
              rect.height > 0 &&
              style.display !== "none" &&
              style.visibility !== "hidden" &&
              !element.disabled
            );
          }) ?? candidates[0];
        if (!visibleCandidate) {
          return false;
        }
        visibleCandidate.focus();
        visibleCandidate.value = "";
        visibleCandidate.dispatchEvent(new Event("input", { bubbles: true }));
        visibleCandidate.value = String(value ?? "");
        visibleCandidate.dispatchEvent(new Event("input", { bubbles: true }));
        visibleCandidate.dispatchEvent(new Event("change", { bubbles: true }));
        return true;
      },
      { selectors, value: String(value ?? "") },
    )
    .catch(() => false);
}

export async function submitAuthEmailViaDom(page, email) {
  return page
    .evaluate((value) => {
      const input = document.querySelector(
        'input[type="email"], input[name="email"], input[autocomplete="username"]',
      );
      if (!input) {
        return false;
      }
      const buttons = Array.from(document.querySelectorAll("button"));
      const continueButton = buttons.find((button) => {
        const text = String(button.innerText || button.textContent || "").trim();
        return (
          text === "Continue" ||
          text === "继续" ||
          text === "Continue with email" ||
          text === "使用电子邮件继续"
        );
      });
      const nativeSetter = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set;
      input.focus();
      if (nativeSetter) {
        nativeSetter.call(input, "");
        nativeSetter.call(input, String(value || ""));
      } else {
        input.value = "";
        input.value = String(value || "");
      }
      input.dispatchEvent(new Event("input", { bubbles: true }));
      input.dispatchEvent(new Event("change", { bubbles: true }));
      continueButton?.click();
      return true;
    }, email)
    .catch(() => false);
}
