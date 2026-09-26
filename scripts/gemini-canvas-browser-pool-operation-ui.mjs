export function createOperationUiOwner({ operationConfig, operationNavLabel, dismissGeminiAppInterstitials, log }) {
  async function clickOperationMode(page, operation, timeoutMs) {
    const config = operationConfig(operation);
    if (!config.buttonName) {
      return false;
    }
    const navLabel = operationNavLabel(operation);
    try {
      await dismissGeminiAppInterstitials(page, timeoutMs);
      if (await operationModeAppearsSelected(page, config, Math.min(timeoutMs, 8_000))) {
        return true;
      }
    } catch {
      // keep trying explicit mode buttons below
    }
    const candidates = [
      page.getByRole("button", { name: config.buttonName }).first(),
      page.getByRole("link", { name: config.buttonName }).first(),
      page.getByRole("menuitem", { name: config.buttonName }).first(),
      page.locator(`[aria-label*="${operation}"], [title*="${operation}"]`).first(),
      page.getByText(config.buttonName).first(),
      ...(navLabel
        ? [
            page.getByRole("link", { name: navLabel }).first(),
            page.getByRole("button", { name: navLabel }).first(),
            page.locator('[aria-label*="Images"], [aria-label*="Videos"]').first(),
          ]
        : []),
    ];

    for (const candidate of candidates) {
      try {
        await dismissGeminiAppInterstitials(page, timeoutMs);
        await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 8_000) });
        await candidate.click({ timeout: Math.min(timeoutMs, 15_000), force: true });
        await page.waitForTimeout(1200);
        if (await operationModeAppearsSelected(page, config, timeoutMs)) {
          return true;
        }
      } catch {
        // keep trying other selectors
      }
    }

    if (await openOperationModeSelector(page, timeoutMs)) {
      const selectorCandidates = [
        page.getByRole("menuitem", { name: config.buttonName }).first(),
        page.getByRole("button", { name: config.buttonName }).first(),
        page.getByRole("link", { name: config.buttonName }).first(),
        page.locator('button,[role="button"],a,[role="menuitem"],div,span').filter({ hasText: config.buttonName }).first(),
        page.getByText(config.buttonName).first(),
      ];
      for (const candidate of selectorCandidates) {
        try {
          await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 8_000) });
          await candidate.click({ timeout: Math.min(timeoutMs, 15_000), force: true });
          await page.waitForTimeout(1200);
          if (await operationModeAppearsSelected(page, config, timeoutMs)) {
            return true;
          }
        } catch {
          // try next menu candidate
        }
      }
      try {
        const fallbackClicked = await page.evaluate(({ pattern, flags }) => {
          const matcher = new RegExp(pattern, flags);
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
            return (
              node.matches('button,[role="button"],a,[role="menuitem"],[tabindex]') ||
              typeof node.onclick === "function" ||
              window.getComputedStyle(node).cursor === "pointer"
            );
          };
          const clickNode = (node) => {
            node.scrollIntoView({ block: "center", inline: "center" });
            node.dispatchEvent(new MouseEvent("pointerdown", { bubbles: true, cancelable: true }));
            node.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true }));
            node.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true }));
            node.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
          };
          const nodes = Array.from(
            document.querySelectorAll('button,[role="button"],a,[role="menuitem"],div,span'),
          );
          for (const node of nodes) {
            const text = (node.innerText || "").trim();
            if (!text || !matcher.test(text) || !isVisible(node)) {
              continue;
            }
            let clickableNode = node;
            let depth = 0;
            while (clickableNode && depth < 8 && !isClickable(clickableNode)) {
              clickableNode = clickableNode.parentElement;
              depth += 1;
            }
            if (!clickableNode || !isVisible(clickableNode)) {
              continue;
            }
            clickNode(clickableNode);
            return {
              clicked: true,
              text,
              targetTag: clickableNode.tagName || null,
              targetRole: clickableNode.getAttribute?.("role") || null,
              targetAria: clickableNode.getAttribute?.("aria-label") || null,
            };
          }
          return { clicked: false };
        }, { pattern: config.buttonName.source, flags: config.buttonName.flags });
        if (fallbackClicked?.clicked) {
          log("media operation mode dom fallback clicked", JSON.stringify({ operation, fallbackClicked }));
          await page.waitForTimeout(1200);
          if (await operationModeAppearsSelected(page, config, timeoutMs)) {
            return true;
          }
        }
      } catch {
        // fall through to final false below
      }
    }

    return false;
  }

  async function clickMediaActionButton(page, operation, action, timeoutMs) {
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
        // try next candidate
      }
    }

    return false;
  }

  async function trySelectMusicStyleCard(page, timeoutMs, attemptIndex = 0) {
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

  async function openOperationModeSelector(page, timeoutMs) {
    const selectorCandidates = [
      page.getByRole("button", { name: /工具|Tools/i }).first(),
      page.locator('button,[role="button"],a,[role="tab"]').filter({ hasText: /工具|Tools/i }).first(),
      page.getByRole("button", { name: /打开模式选择器|Open mode selector/i }).first(),
      page.getByRole("button", { name: /快速|Fast/i }).first(),
      page.getByRole("button", { name: /模式选择器|Mode selector/i }).first(),
    ];
    for (const candidate of selectorCandidates) {
      try {
        await dismissGeminiAppInterstitials(page, timeoutMs);
        await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 5_000) });
        await candidate.click({ timeout: Math.min(timeoutMs, 10_000), force: true });
        await page.waitForTimeout(800);
        return true;
      } catch {
        // try next selector
      }
    }
    return false;
  }

  async function operationModeAppearsSelected(page, config, timeoutMs) {
    const selectedButtonName = config?.selectedButtonName ?? null;
    const modeIndicator = config?.modeIndicator ?? null;
    const routePathPattern = config?.routePathPattern ?? null;
    if (!selectedButtonName && !modeIndicator) {
      return true;
    }
    const deadline = Date.now() + Math.min(timeoutMs, 8_000);
    while (Date.now() < deadline) {
      if (routePathPattern && typeof page?.url === "function") {
        try {
          if (routePathPattern.test(String(page.url() || ""))) {
            return true;
          }
        } catch {
          // ignore and continue
        }
      }
      if (selectedButtonName) {
        const selectedCandidates = [
          page.getByRole("button", { name: selectedButtonName }).first(),
          page.getByRole("link", { name: selectedButtonName }).first(),
        ];
        for (const candidate of selectedCandidates) {
          try {
            if (await candidate.isVisible({ timeout: 300 })) {
              return true;
            }
          } catch {
            // ignore and continue
          }
        }
      }
      try {
        const bodyText = await page.evaluate(() => document.body?.innerText ?? "");
        if (modeIndicator.test(String(bodyText || ""))) {
          return true;
        }
      } catch {
        // ignore and retry
      }
      await page.waitForTimeout(350);
    }
    return false;
  }

  return { clickOperationMode, clickMediaActionButton, trySelectMusicStyleCard, openOperationModeSelector, operationModeAppearsSelected };
}
