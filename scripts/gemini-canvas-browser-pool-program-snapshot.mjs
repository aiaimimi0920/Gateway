import { extractProgramHandleHintsFromText } from "./gemini-canvas-browser-pool-program-handles.mjs";

export async function collectProgramHandleSnapshot(page) {
  const snapshot = await page.evaluate(() => {
    const bodyText = document.body?.innerText ?? "";
    const buttons = Array.from(
      document.querySelectorAll('button,[role="button"],a[role="button"]'),
    )
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 200);
    const anchors = Array.from(document.querySelectorAll("a[href]"))
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        href: node.href,
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
        target: node.getAttribute("target"),
        download: node.getAttribute("download"),
      }))
      .filter((entry) => entry.href)
      .slice(0, 200);
    const mediaNodes = [
      ...Array.from(document.querySelectorAll("audio")).map((node, index) => ({
        kind: "audio",
        index,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
      })),
      ...Array.from(document.querySelectorAll("video")).map((node, index) => ({
        kind: "video",
        index,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
        width: node.videoWidth,
        height: node.videoHeight,
        poster: node.getAttribute("poster"),
      })),
      ...Array.from(document.querySelectorAll("img")).map((node, index) => ({
        kind: "image",
        index,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        alt: node.getAttribute("alt"),
        width: node.naturalWidth,
        height: node.naturalHeight,
      })),
    ]
      .filter((entry) => entry.src || entry.currentSrc)
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
      .slice(0, 80);
    let historyState;
    try {
      historyState = JSON.stringify(history.state ?? null);
    } catch {
      historyState = null;
    }
    return {
      url: location.href,
      title: document.title,
      bodyText: (bodyText || "").slice(0, 12000),
      historyState: typeof historyState === "string" ? historyState.slice(0, 4000) : null,
      buttons,
      anchors,
      mediaNodes,
      textboxes,
    };
  });

  const handleHints = extractProgramHandleHintsFromText(
    [
      snapshot.url,
      snapshot.historyState,
      snapshot.bodyText,
      ...(snapshot.anchors || []).flatMap((anchor) => [
        anchor.href,
        anchor.text,
        anchor.ariaLabel,
        anchor.title,
      ]),
    ]
      .filter(Boolean)
      .join("\n"),
  );

  return {
    ...snapshot,
    handleHints,
  };
}
