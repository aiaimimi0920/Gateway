// Reactive media editors need native locator input; only readiness may be retried.
export async function fillAndSubmitMediaPrompt(page, textbox, prompt, timeoutMs, buildSendCandidates) {
  await textbox.fill(prompt, { timeout: Math.min(timeoutMs, 30_000) });
  const sendCandidates = buildSendCandidates();
  const deadline = Date.now() + Math.min(timeoutMs, 10_000);
  do {
    for (const candidate of sendCandidates.slice(0, 2)) {
      if (!await candidate.isVisible() || !await candidate.isEnabled()) continue;
      // A rejected click may already have submitted. Never catch it to try another send.
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000) });
      return;
    }
    if (Date.now() >= deadline) break;
    await page.waitForTimeout(Math.min(100, deadline - Date.now()));
  } while (Date.now() <= deadline);
  throw Object.assign(new Error("Media send button is unavailable; no prompt was submitted."), {
    status: 409, code: "gemini_web_media_submit_unavailable",
  });
}
