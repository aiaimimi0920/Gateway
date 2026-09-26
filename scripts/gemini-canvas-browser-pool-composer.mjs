export async function submitPrompt(page, prompt, timeoutMs) {
  const textbox = page
    .locator(
      '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
    )
    .first();
  await textbox.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 30_000) });
  await textbox.focus();
  let populated = false;
  let prefersKeyboardTyping = false;
  try {
    const populateResult = await textbox.evaluate((node, value) => {
      const textValue = String(value ?? "");
      const fireInput = () =>
        node.dispatchEvent(
          new InputEvent("input", {
            bubbles: true,
            cancelable: true,
            data: textValue,
            inputType: "insertText",
          }),
        );
      if (node instanceof HTMLInputElement || node instanceof HTMLTextAreaElement) {
        node.value = textValue;
        fireInput();
        node.dispatchEvent(new Event("change", { bubbles: true }));
        return {
          populated: true,
          editorKind: "native_input",
        };
      }
      if (node instanceof HTMLElement && node.isContentEditable) {
        node.innerHTML = "";
        node.textContent = textValue;
        fireInput();
        return {
          populated: true,
          editorKind: "contenteditable",
        };
      }
      return {
        populated: false,
        editorKind: "unknown",
      };
    }, prompt);
    if (populateResult && typeof populateResult === "object") {
      populated = Boolean(populateResult.populated);
      prefersKeyboardTyping = populateResult.editorKind === "contenteditable";
    } else {
      populated = Boolean(populateResult);
    }
  } catch {
    populated = false;
  }

  if (!populated || prefersKeyboardTyping) {
    await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
    await page.keyboard.press("Backspace").catch(() => undefined);
    await page.keyboard.type(prompt, { delay: 14 });
  }
  await page.waitForTimeout(300);

  const sendCandidates = buildSendButtonCandidates(page);

  let clicked = false;
  for (const candidate of sendCandidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 4_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000), force: true });
      clicked = true;
      break;
    } catch {
      // try the next candidate
    }
  }

  if (!clicked) {
    const modifier = process.platform === "win32" ? "Control" : "Meta";
    await page.keyboard.press(`${modifier}+Enter`).catch(() => undefined);
    await page.waitForTimeout(200);
    await page.keyboard.press("Enter").catch(() => undefined);
  }
}

export async function tryClickSendButton(page, timeoutMs) {
  const sendCandidates = buildSendButtonCandidates(page);

  for (const candidate of sendCandidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 1_500) });
      await candidate.click({ timeout: Math.min(timeoutMs, 5_000), force: true });
      return true;
    } catch {
      // try next candidate
    }
  }
  return false;
}

export function buildSendButtonCandidates(page) {
  return [
    page.getByRole("button", { name: /发送|Send|提交|Submit/i }).first(),
    page.locator(
      'button[aria-label*="Send"], button[aria-label*="发送"], button[aria-label*="Submit"], button[aria-label*="提交"], button[title*="Send"], button[title*="发送"], button[title*="Submit"], button[title*="提交"]',
    ).first(),
    page.locator('button:has(svg), button:has(i)').last(),
  ];
}
