import { tryClickSendButton } from "./gemini-canvas-browser-pool-composer.mjs";

export function bodyTextSuggestsVideoTemplateSelection(bodyText) {
  const text = String(bodyText || "");
  if (!text) {
    return false;
  }
  return (
    /挑选一个模板/.test(text) ||
    /开始制作你的视频/.test(text) ||
    /choose a template/i.test(text) ||
    /start creating your video/i.test(text)
  );
}

export async function hasVideoCreateAction(page, timeoutMs) {
  const candidates = [
    page
      .locator('button,[role="button"],a[role="button"],div[role="button"],span[role="button"]')
      .filter({ hasText: /创作视频|制作视频|Create video/i })
      .last(),
  ];

  for (const candidate of candidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 800) });
      return true;
    } catch {
      // try next candidate
    }
  }

  try {
    return await page.evaluate(() => {
      const matcher = /创作视频|制作视频|Create video/i;
      return Array.from(
        document.querySelectorAll(
          'button,[role="button"],a[role="button"],div[role="button"],span[role="button"]',
        ),
      ).some((node) => {
        const text = `${node.textContent || ""}\n${node.getAttribute?.("aria-label") || ""}\n${node.getAttribute?.("title") || ""}`;
        if (!matcher.test(text)) {
          return false;
        }
        const style = window.getComputedStyle(node);
        const rect = node.getBoundingClientRect();
        return (
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          rect.width > 0 &&
          rect.height > 0
        );
      });
    });
  } catch {
    return false;
  }
}

export async function tryClickVideoCreateAction(page, timeoutMs) {
  const candidates = [
    page
      .locator('button,[role="button"],a[role="button"],div[role="button"],span[role="button"]')
      .filter({ hasText: /创作视频|制作视频|Create video/i })
      .last(),
  ];

  for (const candidate of candidates) {
    try {
      await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 1_500) });
      const ariaLabel = await candidate.getAttribute("aria-label").catch(() => null);
      const title = await candidate.getAttribute("title").catch(() => null);
      const combined = `${ariaLabel || ""}\n${title || ""}`;
      if (/取消选择|deselect/i.test(combined)) {
        continue;
      }
      await candidate.click({ timeout: Math.min(timeoutMs, 5_000), force: true });
      return true;
    } catch {
      // try next candidate
    }
  }

  try {
    const domClicked = await page.evaluate(() => {
      const matcher = /创作视频|制作视频|Create video/i;
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
      const dispatchClick = (node) => {
        node.scrollIntoView({ block: "center", inline: "center" });
        for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
          node.dispatchEvent(
            new MouseEvent(type, {
              bubbles: true,
              cancelable: true,
              composed: true,
              view: window,
            }),
          );
        }
        if (typeof node.click === "function") {
          node.click();
        }
      };

      const nodes = Array.from(
        document.querySelectorAll(
          'button,[role="button"],a[role="button"],div[role="button"],span[role="button"]',
        ),
      );
      for (const node of nodes) {
        const combined = `${node.textContent || ""}\n${node.getAttribute?.("aria-label") || ""}\n${node.getAttribute?.("title") || ""}`;
        if (!matcher.test(combined) || /取消选择|deselect/i.test(combined) || !isVisible(node)) {
          continue;
        }
        let clickableNode = node;
        let depth = 0;
        while (clickableNode && depth < 8) {
          if (isClickable(clickableNode) && isVisible(clickableNode)) {
            break;
          }
          clickableNode = clickableNode.parentElement;
          depth += 1;
        }
        dispatchClick(clickableNode instanceof HTMLElement ? clickableNode : node);
        return true;
      }
      return false;
    });
    if (domClicked) {
      return true;
    }
  } catch {
    // fall through
  }
  return false;
}

export async function trySelectVideoTemplateCard(page, timeoutMs, attemptIndex = 0) {
  const selection = await page.evaluate((requestedIndex) => {
    const templateLabels = [
      "slime",
      "Civilization",
      "Metallic",
      "Memo",
      "Glam",
      "Crochet",
      "Cyberpunk",
      "Video Game",
      "Cosmos",
      "Action Hero",
      "Stardust",
      "Jellytoon",
      "Racetrack",
      "ASMR Apple",
      "Red Carpet",
      "Popcorn",
    ];
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

    const templateImages = Array.from(document.querySelectorAll("img"))
      .filter((node) => /视频生成模板图片|video generation template/i.test(node.alt || ""))
      .filter((node) => isVisible(node))
      .map((node) => ({
        source: "image",
        node,
        label: node.alt || null,
      }));
    const templateTextNodes = Array.from(document.querySelectorAll("button,[role=\"button\"],a,div,span"))
      .map((node) => ({
        node,
        text: (node.innerText || "").trim(),
      }))
      .filter((entry) => entry.text && templateLabels.includes(entry.text))
      .filter((entry) => isVisible(entry.node))
      .map((entry) => ({
        source: "text",
        node: entry.node,
        label: entry.text,
      }));
    const templateCandidates = [...templateImages, ...templateTextNodes];
    if (templateCandidates.length === 0) {
      return {
        clicked: false,
        reason: "template_candidate_missing",
        templateCount: 0,
      };
    }

    const selectedIndex = Math.min(
      Math.max(Number(requestedIndex || 0), 0),
      templateCandidates.length - 1,
    );
    const selectedCandidate = templateCandidates[selectedIndex];
    const imageNode = selectedCandidate.node;
    let clickableNode = findClickableAncestor(imageNode);
    let depth = 0;
    while (clickableNode && depth < 8) {
      if (isClickable(clickableNode) && isVisible(clickableNode)) {
        break;
      }
      clickableNode = clickableNode.parentElement;
      depth += 1;
    }

    if (!(clickableNode instanceof HTMLElement) || !isVisible(clickableNode)) {
      clickableNode = imageNode;
    }

    clickNode(clickableNode);

    return {
      clicked: true,
      reason: "template_clicked",
      templateCount: templateCandidates.length,
      selectedIndex,
      selectedSource: selectedCandidate.source,
      selectedLabel: selectedCandidate.label,
      selectedSrc: imageNode.currentSrc || imageNode.src || null,
      selectedAlt: imageNode.alt || null,
      clickedTag: clickableNode.tagName || null,
      clickedRole: clickableNode.getAttribute?.("role") || null,
      clickedAria: clickableNode.getAttribute?.("aria-label") || null,
      clickedText: (clickableNode.innerText || "").trim().slice(0, 200),
    };
  }, attemptIndex);

  await page.waitForTimeout(1_500);
  const videoCreateVisible = await hasVideoCreateAction(page, timeoutMs).catch(() => false);
  const videoCreateClicked = await tryClickVideoCreateAction(page, timeoutMs).catch(
    () => false,
  );
  const sendClicked = !videoCreateClicked && !videoCreateVisible
    ? await tryClickSendButton(page, timeoutMs).catch(() => false)
    : false;
  return {
    ...selection,
    videoCreateVisible,
    videoCreateClicked,
    sendClicked,
  };
}
