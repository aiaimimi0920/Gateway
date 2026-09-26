import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { parseBoolean } from "./gemini-canvas-browser-pool-executable.mjs";
import { collectButtonSnapshot, collectPageSnapshot } from "./gemini-canvas-browser-pool-page-snapshot.mjs";
import { extractProgramHandleHintsFromText, mergeProgramHandleHints } from "./gemini-canvas-browser-pool-program-handles.mjs";

export function createDebugOperationOwner({
  DEFAULT_TIMEOUT_MS, resolveProgramPageUrl, resetConversation, startNetworkCapture,
  clickOperationMode, log, buildProgramHandleState,
}) {
  async function runDebugOperation(entry, args = {}) {
    const { page } = entry;
    const baseUrl = normalizeString(args.baseUrl) ?? "https://gemini.google.com";
    const timeoutMs = Math.max(
      Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
      DEFAULT_TIMEOUT_MS,
    );
    const clickOperation = normalizeString(args.clickOperation);
    const captureNetwork = parseBoolean(args.captureNetwork, false);
    const networkWaitMs = Math.max(Number(args.networkWaitMs || 5000), 0);
    const preferredProgramPageUrl = resolveProgramPageUrl(baseUrl, args);

    if (args.resetConversation !== false) {
      await resetConversation(page, baseUrl, timeoutMs, preferredProgramPageUrl);
    }
    let capture = null;
    if (captureNetwork) {
      capture = startNetworkCapture(page, clickOperation || "text");
    }
    try {
      await capture?.ready;
      if (clickOperation) {
        const selected = await clickOperationMode(page, clickOperation, timeoutMs);
        log("debug click operation result", JSON.stringify({ clickOperation, selected }));
        await page.waitForTimeout(captureNetwork ? Math.max(networkWaitMs, 1200) : 1200);
      }

      const snapshot = await collectPageSnapshot(page);
      const buttons = await collectButtonSnapshot(page).catch(() => []);
      const matchingButtonHtml = clickOperation
        ? await page
            .evaluate((operation) => {
              const candidates = Array.from(document.querySelectorAll("button, [role=\"button\"], a"));
              const matcher =
                operation === "image"
                  ? /制作图片|Create image|Create images|Make image/i
                  : operation === "music"
                    ? /创作音乐|制作音乐|Create music/i
                    : operation === "video"
                      ? /创作视频|制作视频|Create video/i
                      : null;
              if (!matcher) {
                return [];
              }
              return candidates
                .filter((node) => matcher.test(node.textContent || "") || matcher.test(node.getAttribute?.("aria-label") || ""))
                .slice(0, 6)
                .map((node, index) => ({
                  index,
                  outerHTML: node.outerHTML.slice(0, 4000),
                  text: (node.textContent || "").trim().slice(0, 300),
                  ariaLabel: node.getAttribute?.("aria-label") || null,
                }));
            }, clickOperation)
            .catch(() => [])
        : [];
      const textboxes = await page
        .evaluate(() =>
          Array.from(
            document.querySelectorAll(
              '[role="textbox"], textarea, input[type="text"], [contenteditable="true"]',
            ),
          )
            .map((node, index) => ({
              index,
              tagName: node.tagName,
              ariaLabel: node.getAttribute?.("aria-label") || null,
              placeholder: node.getAttribute?.("placeholder") || null,
              text: (node.innerText || node.textContent || node.value || "").trim().slice(0, 300),
            }))
            .slice(0, 80),
        )
        .catch(() => []);
      const pageDiagnostics = await page.evaluate(() => {
        const html = document.documentElement?.outerHTML || "";
        const scripts = Array.from(document.scripts || []).map((node) => node.textContent || "").join("\n");
        const blob = `${html}\n${scripts}`;
        const apiKeys = Array.from(new Set(blob.match(/AIza[0-9A-Za-z\-_]{20,}/g) || []));
        const debugTerms = [
          "generativelanguage.googleapis.com",
          "clients6.google.com",
          "tts",
          "speech",
          "voiceConfig",
          "responseModalities",
        ];
        const snippets = {};
        for (const term of debugTerms) {
          const loweredBlob = blob.toLowerCase();
          const loweredTerm = term.toLowerCase();
          const index = loweredBlob.indexOf(loweredTerm);
          if (index >= 0) {
            const start = Math.max(0, index - 240);
            const end = Math.min(blob.length, index + loweredTerm.length + 240);
            snippets[term] = blob.slice(start, end);
          }
        }
        const interesting = {};
        for (const key of Object.keys(window)) {
          if (!/config|bootstrap|data|init|api|key/i.test(key)) {
            continue;
          }
          try {
            const value = window[key];
            const serialized = typeof value === "string" ? value : JSON.stringify(value);
            if (serialized && serialized.includes("AIza")) {
              interesting[key] = serialized.slice(0, 4000);
            }
          } catch {
            // ignore serialization failures for opaque browser-owned objects
          }
        }
        return {
          url: location.href,
          title: document.title,
          apiKeys,
          snippets,
          interestingKeys: interesting,
          bodyText: document.body?.innerText ?? "",
        };
        });

      const networkEvents = capture?.state?.events ?? [];
      const domHandleHints = extractProgramHandleHintsFromText(
        await page.content().catch(() => ""),
      );
      if (capture?.state?.handleHints) {
        mergeProgramHandleHints(capture.state.handleHints, domHandleHints);
      }
      const programHandleState = buildProgramHandleState(
        baseUrl,
        args,
        pageDiagnostics.url,
        capture?.state,
      );

      return {
        operation: "debug",
        pageUrl: pageDiagnostics.url,
        ...programHandleState,
        title: pageDiagnostics.title,
        bodyText: pageDiagnostics.bodyText,
        buttons,
        textboxes,
        matchingButtonHtml,
        media: snapshot.mediaNodes || [],
        apiKeys: pageDiagnostics.apiKeys || [],
        snippets: pageDiagnostics.snippets || {},
        interestingKeys: pageDiagnostics.interestingKeys || {},
        networkEvents,
        rpcCaptures: capture?.state?.rpcCaptures ?? [],
      };
    } finally {
      await capture?.stop?.();
    }
  }

  return { runDebugOperation };
}
