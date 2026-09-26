async function collectPageSnapshot(page, label) {
  const snapshot = await page.evaluate((phase) => {
    const bodyText = document.body?.innerText ?? "";
    const buttons = Array.from(
      document.querySelectorAll("button,[role=\"button\"],a[role=\"button\"]"),
    )
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 120);
    const textboxes = Array.from(
      document.querySelectorAll('[role="textbox"], textarea, [contenteditable="true"], input'),
    )
      .map((node, index) => ({
        index,
        tag: node.tagName,
        role: node.getAttribute("role"),
        placeholder: node.getAttribute("placeholder"),
        ariaLabel: node.getAttribute("aria-label"),
      }))
      .slice(0, 60);
    const iframes = Array.from(document.querySelectorAll("iframe"))
      .map((node, index) => ({
        index,
        src: node.getAttribute("src"),
        title: node.getAttribute("title"),
        ariaLabel: node.getAttribute("aria-label"),
      }))
      .slice(0, 40);
    const hookEvents = Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)
      ? window.__AISTUDIO_LIVE_CAPTURE__.slice(-60)
      : [];
    return {
      label: phase,
      url: location.href,
      title: document.title,
      bodyText: bodyText.slice(0, 12000),
      buttons,
      textboxes,
      iframes,
      hookEvents,
    };
  }, label);
  return snapshot;
}

async function collectFrameDiagnostics(page, label) {
  const frames = [];
  for (const frame of page.frames()) {
    try {
      const data = await frame.evaluate((phase) => {
        const hookEvents = Array.isArray(window.__AISTUDIO_LIVE_CAPTURE__)
          ? window.__AISTUDIO_LIVE_CAPTURE__.slice(-120)
          : [];
        return {
          label: phase,
          url: location.href,
          title: document.title,
          name: window.name || null,
          hookEvents,
        };
      }, label);
      frames.push(data);
    } catch (error) {
      frames.push({
        label,
        url: frame.url(),
        name: frame.name() || null,
        error: String(error),
      });
    }
  }
  return frames;
}

export { collectPageSnapshot, collectFrameDiagnostics };
