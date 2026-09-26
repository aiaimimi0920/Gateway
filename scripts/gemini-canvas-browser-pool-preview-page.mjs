export function createPreviewPageOwner({
  ensureSharePage, tryFollowShareEntryPoint, tryOpenCanvasProxyPreview,
  stampCanvasProxyPreviewFrames, inferGoogleAuthUser, log,
}) {
  function isCanvasProxyPreviewFrameUrl(url) {
    return /scf\.usercontent\.goog|^blob:/i.test(String(url || ""));
  }

  async function ensureCanvasProxyPreviewFrame(
    entry,
    baseUrl,
    shareId,
    timeoutMs,
    previewFetchRequest = null,
  ) {
    const adoptPage = async (nextPage) => {
      if (!nextPage || nextPage === entry.page) {
        return;
      }
      const previousPage = entry.page;
      entry.page = nextPage;
      if (previousPage && previousPage !== nextPage && !previousPage.isClosed()) {
        await previousPage.close().catch(() => undefined);
      }
    };

    await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
    let activePage = entry.page;
    let shareFollow = await tryFollowShareEntryPoint(activePage);
    if (shareFollow.page) {
      await adoptPage(shareFollow.page);
      activePage = entry.page;
    }
    await activePage.waitForTimeout(2500);

    let bodyText = await activePage
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
    if (!/Browser API Proxy Client/i.test(bodyText)) {
      await ensureSharePage(entry, baseUrl, shareId, timeoutMs);
      activePage = entry.page;
      shareFollow = await tryFollowShareEntryPoint(activePage);
      if (shareFollow.page) {
        await adoptPage(shareFollow.page);
        activePage = entry.page;
      }
      await activePage.waitForTimeout(2500);
      bodyText = await activePage
        .evaluate(() => document.body?.innerText ?? "")
        .catch(() => "");
    }

    const preview = await tryOpenCanvasProxyPreview(activePage, timeoutMs);
    log(
      "canvas proxy preview open result",
      JSON.stringify({
        clicked: preview?.clicked === true,
        reason: preview?.reason ?? null,
        pageUrl: activePage.url(),
        bodyPreview: String(preview?.bodyPreview ?? bodyText ?? "").slice(0, 500),
        frameCount: activePage.frames().length,
      }),
    );
    if (preview?.page) {
      await adoptPage(preview.page);
      activePage = entry.page;
    }
    await activePage.waitForTimeout(1200);

    const previewFrameDeadline = Date.now() + Math.min(Math.max(timeoutMs, 8000), 20000);
    let lastStampedFrames = [];
    let lastBodyText = bodyText;
    while (Date.now() < previewFrameDeadline) {
      lastStampedFrames = await stampCanvasProxyPreviewFrames(
        activePage,
        inferGoogleAuthUser(activePage.url()),
        previewFetchRequest,
      );
      const frames = activePage.frames();
      const preferredStampedFrame = lastStampedFrames.find(
        (entry) => entry?.stamped && entry?.probeFetchResult?.ok && frames[entry.index],
      );
      if (preferredStampedFrame) {
        return {
          page: activePage,
          frame: frames[preferredStampedFrame.index],
          preview,
          stampedFrames: lastStampedFrames,
        };
      }
      const fallbackStampedFrame = lastStampedFrames.find(
        (entry) => entry?.stamped && frames[entry.index],
      );
      if (fallbackStampedFrame) {
        return {
          page: activePage,
          frame: frames[fallbackStampedFrame.index],
          preview,
          stampedFrames: lastStampedFrames,
        };
      }
      const fallbackFrame = frames.find(
        (frame, index) => index > 0 && isCanvasProxyPreviewFrameUrl(frame.url()),
      );
      if (fallbackFrame) {
        return {
          page: activePage,
          frame: fallbackFrame,
          preview,
          stampedFrames: lastStampedFrames,
        };
      }
      await activePage.waitForTimeout(1000);
      lastBodyText = await activePage
        .evaluate(() => document.body?.innerText ?? "")
        .catch(() => lastBodyText);
    }
    throw Object.assign(
      new Error("Gemini Canvas preview frame was not available for no-key invocation."),
      {
        status: 503,
        code: "gemini_canvas_preview_frame_missing",
        bodyText: String(lastBodyText || "").slice(0, 800),
      },
    );
  }

  return { isCanvasProxyPreviewFrameUrl, ensureCanvasProxyPreviewFrame };
}
