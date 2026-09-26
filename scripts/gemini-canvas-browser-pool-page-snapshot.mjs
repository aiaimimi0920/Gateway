export async function collectButtonSnapshot(page) {
  return page.evaluate(() =>
    Array.from(document.querySelectorAll('button,[role="button"],a[role="button"]'))
      .map((node, index) => ({
        index,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 300),
  );
}

export async function collectPageSnapshot(page) {
  return page.evaluate(() => {
    const buttons = Array.from(
      document.querySelectorAll('button,[role="button"],a[role="button"]'),
    )
      .map((node, i) => ({
        i,
        text: (node.innerText || "").trim(),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
      }))
      .filter((entry) => entry.text || entry.ariaLabel || entry.title)
      .slice(0, 200);
    const mediaNodes = [
      ...Array.from(document.querySelectorAll("audio")).map((node, i) => ({
        kind: "audio",
        i,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        duration: Number.isFinite(node.duration) ? node.duration : null,
      })),
      ...Array.from(document.querySelectorAll("video")).map((node, i) => ({
        kind: "video",
        i,
        src: node.getAttribute("src"),
        currentSrc: node.currentSrc,
        controls: node.controls,
        width: node.videoWidth,
        height: node.videoHeight,
        duration: Number.isFinite(node.duration) ? node.duration : null,
        poster: node.getAttribute("poster"),
      })),
      ...Array.from(document.querySelectorAll("img")).map((node, i) => ({
        kind: "image",
        i,
        alt: node.getAttribute("alt"),
        src: node.getAttribute("src"),
        width: node.naturalWidth,
        height: node.naturalHeight,
      })),
    ].filter((node) => node.src || node.currentSrc);

    const anchorNodes = Array.from(document.querySelectorAll("a"))
      .map((node, i) => ({
        i,
        text: (node.textContent || "").trim(),
        href: node.href,
        rawHref: node.getAttribute("href"),
        ariaLabel: node.getAttribute("aria-label"),
        title: node.getAttribute("title"),
        target: node.getAttribute("target"),
        download: node.getAttribute("download"),
      }))
      .filter((node) => node.href || node.text);

    return {
      buttons,
      mediaNodes,
      anchorNodes,
      pageState: {
        url: location.href,
        title: document.title,
        bodyText: (document.body?.innerText ?? "").slice(0, 12000),
      },
    };
  });
}
