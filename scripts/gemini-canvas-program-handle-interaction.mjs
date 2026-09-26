import { hasPromptTextbox, clickFirstVisible } from "./gemini-canvas-browser-pool-app.mjs";
export { hasPromptTextbox, clickFirstVisible };

export async function waitForShareSurface(page, deadlineMs) {
  const deadline = Date.now() + deadlineMs;
  while (Date.now() < deadline) {
    const bodyText = await page.evaluate(() => (document.body?.innerText ?? "").slice(0, 4000));
    if (/继续|试用 Gemini Canvas|在新窗口中打开|不要公开个人信息/i.test(bodyText)) {
      return true;
    }
    await page.waitForTimeout(1200);
  }
  return false;
}

export async function tryFollowShareEntryPoint(page) {
  const candidates = [
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /试用 Gemini Canvas|Try Gemini Canvas/i }).first(),
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /继续|Continue/i }).first(),
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /制作图片|Create image|Create images|Make image/i }).first(),
    page.locator("a,button,[role=\"button\"]").filter({ hasText: /在新窗口中打开|Open in new window/i }).first(),
  ];

  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) === 0) {
        continue;
      }
      await candidate.waitFor({ state: "visible", timeout: 5000 });
      const popupPromise = page.waitForEvent("popup", { timeout: 6000 }).catch(() => null);
      await candidate.click({ timeout: 12000, force: true });
      const popup = await popupPromise;
      if (popup) {
        await popup.waitForLoadState("domcontentloaded", { timeout: 20000 }).catch(() => undefined);
        await popup.waitForTimeout(3000);
        return { kind: "popup", page: popup };
      }
      await page.waitForTimeout(3000);
      return { kind: "same_page", page };
    } catch {
      // try next candidate
    }
  }

  return { kind: "none", page };
}

export async function clickNewChat(page) {
  const candidates = [
    page.getByRole("button", { name: /发起新对话|New chat/i }).first(),
    page.locator('button[aria-label*="发起新对话"], button[aria-label*="New chat"]').first(),
    page.getByRole("link", { name: /发起新对话|New chat/i }).first(),
  ];
  const clicked = await clickFirstVisible(candidates);
  if (clicked) {
    await page.waitForTimeout(1200);
  }
  return clicked;
}

export async function clickOperationMode(page, selectedOperation) {
  let matcher = null;
  switch (selectedOperation) {
    case "image":
      matcher = /制作图片|Create image|Create images|Make image/i;
      break;
    case "music":
      matcher = /创作音乐|制作音乐|Create music/i;
      break;
    case "video":
      matcher = /创作视频|制作视频|Create video/i;
      break;
    default:
      return false;
  }
  const candidates = [
    page.locator("a,button,[role=\"button\"]").filter({ hasText: matcher }).first(),
    page.getByRole("button", { name: matcher }).first(),
  ];
  const clicked = await clickFirstVisible(candidates);
  if (clicked) {
    await page.waitForTimeout(1500);
  }
  return clicked;
}

export async function clickMediaActionButton(page, operation, action, timeoutMs) {
  let matcher = null;
  if (operation === "music" && action === "download") {
    matcher = /下载音乐作品|Download music/i;
  } else if (operation === "music" && action === "play") {
    matcher = /播放视频|播放|Play/i;
  } else if (operation === "video" && action === "download") {
    matcher = /下载视频|Download video/i;
  } else if (operation === "video" && action === "play") {
    matcher = /播放视频|Play video/i;
  } else {
    return false;
  }
  const candidates = [
    page.getByRole("button", { name: matcher }).first(),
    page.locator("button,[role=\"button\"],a").filter({ hasText: matcher }).first(),
  ];
  for (const candidate of candidates) {
    try {
      if ((await candidate.count()) === 0) {
        continue;
      }
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 5_000) });
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000), force: true });
      await page.waitForTimeout(1500);
      return true;
    } catch {
      // try next
    }
  }
  return false;
}

export async function trySelectMusicStyleCard(page, timeoutMs, attemptIndex = 0) {
  const selection = await page.evaluate((requestedIndex) => {
    const isVisible = (node) => {
      if (!(node instanceof HTMLElement)) {
        return false;
      }
      const style = window.getComputedStyle(node);
      if (style.display === "none" || style.visibility === "hidden") {
        return false;
      }
      const rect = node.getBoundingClientRect();
      return rect.width > 0 && rect.height > 0;
    };
    const isClickable = (node) => {
      if (!(node instanceof HTMLElement)) {
        return false;
      }
      const style = window.getComputedStyle(node);
      return (
        node.matches('button,[role="button"],a,[tabindex]') ||
        typeof node.onclick === "function" ||
        style.cursor === "pointer"
      );
    };
    const clickNode = (node) => {
      node.scrollIntoView({ block: "center", inline: "center" });
      const dispatchClick = (targetNode) => {
        targetNode.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true }));
        targetNode.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
        targetNode.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
        targetNode.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
      };
      try {
        if (typeof node.click === "function") {
          node.click();
        } else {
          dispatchClick(node);
        }
        dispatchClick(node);
      } catch {
        dispatchClick(node);
      }
    };
    const findClickableAncestor = (node) => {
      let clickableNode = node;
      let depth = 0;
      while (clickableNode && depth < 8) {
        if (isClickable(clickableNode) && isVisible(clickableNode)) {
          break;
        }
        clickableNode = clickableNode.parentElement;
        depth += 1;
      }
      return clickableNode instanceof HTMLElement && isVisible(clickableNode)
        ? clickableNode
        : node;
    };

    const candidates = Array.from(document.querySelectorAll("img"))
      .filter((node) => isVisible(node))
      .map((node) => ({
        node,
        alt: (node.alt || "").trim(),
        width: node.naturalWidth || 0,
        height: node.naturalHeight || 0,
      }))
      .filter(
        (entry) =>
          entry.alt &&
          !/个人资料照片|profile/i.test(entry.alt) &&
          entry.width >= 300 &&
          entry.height >= 300,
      );
    if (candidates.length === 0) {
      return {
        clicked: false,
        reason: "style_candidate_missing",
        styleCount: 0,
      };
    }

    const selectedIndex = Math.min(Math.max(Number(requestedIndex || 0), 0), candidates.length - 1);
    const selectedCandidate = candidates[selectedIndex];
    const clickableNode = findClickableAncestor(selectedCandidate.node);
    clickNode(clickableNode);
    return {
      clicked: true,
      reason: "style_clicked",
      styleCount: candidates.length,
      selectedIndex,
      selectedAlt: selectedCandidate.alt,
      selectedSrc: selectedCandidate.node.currentSrc || selectedCandidate.node.src || null,
      clickedTag: clickableNode.tagName || null,
      clickedRole: clickableNode.getAttribute?.("role") || null,
      clickedAria: clickableNode.getAttribute?.("aria-label") || null,
      clickedText: (clickableNode.innerText || "").trim().slice(0, 200),
    };
  }, attemptIndex);

  await page.waitForTimeout(1500);
  return selection;
}

export async function submitPrompt(page, value) {
  const textbox = page
    .locator(
      '[role="textbox"][aria-label*="Gemini"], [role="textbox"][aria-label*="输入"], [role="textbox"], [contenteditable="true"]',
    )
    .first();
  await textbox.waitFor({ state: "visible", timeout: 30000 });
  await textbox.focus();
  let populated = false;
  try {
    await textbox.evaluate((node, promptText) => {
      const textValue = String(promptText ?? "");
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
        return true;
      }
      if (node instanceof HTMLElement && node.isContentEditable) {
        node.textContent = textValue;
        fireInput();
        return true;
      }
      return false;
    }, value);
    populated = true;
  } catch {
    populated = false;
  }

  if (!populated) {
    await page.keyboard.press(process.platform === "win32" ? "Control+A" : "Meta+A").catch(() => undefined);
    await page.keyboard.press("Backspace").catch(() => undefined);
    await page.keyboard.type(value, { delay: 12 });
  }
  await page.waitForTimeout(300);

  const sendCandidates = [
    page.getByRole("button", { name: /发送|Send/i }).first(),
    page
      .locator(
        'button[aria-label*="Send"], button[aria-label*="发送"], button[title*="Send"], button[title*="发送"]',
      )
      .first(),
  ];
  const clicked = await clickFirstVisible(sendCandidates);
  if (!clicked) {
    await page.keyboard.press("Enter").catch(() => undefined);
  }
}
