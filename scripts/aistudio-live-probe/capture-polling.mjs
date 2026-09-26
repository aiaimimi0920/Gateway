import { bestEffortDismissAistudioOverlays, bestEffortApplyRemixModal,
  shouldRetryAutoPromptDuringPolling, bestEffortAutoPrompt } from "./ui-actions.mjs";
import { hasPendingLocalProxyResponse, countOpenUndispatchedLocalProxyConnections,
  maybeDispatchLocalProxyMessages } from "./local-dispatch.mjs";

async function pollProbeCapture({
  page, capture, timeoutMs, settleMs, localWebSocketServer, localProxyMessages,
  localProxyDelayMs, normalizedAutoPrompt, autoPromptSubmitted, persistCapture,
  getMatchTimes,
}) {
  const startedAt = Date.now();
  let lastLocalProxyRedispatchAt = 0;
  let lastRemixApplyAt = 0;
  let lastAutoPromptAttemptAt = 0;
  while (Date.now() - startedAt < timeoutMs) {
    const { firstMatchAt, lastMatchAt } = getMatchTimes();
    if (
      firstMatchAt &&
      lastMatchAt &&
      Date.now() - lastMatchAt >= settleMs &&
      !hasPendingLocalProxyResponse(capture)
    ) {
      break;
    }
    if (
      localWebSocketServer &&
      localProxyMessages.length > 0 &&
      countOpenUndispatchedLocalProxyConnections(capture) > 0 &&
      Date.now() - lastLocalProxyRedispatchAt >= Math.max(localProxyDelayMs, 1_500)
    ) {
      // Some run.app previews tear down the first local bridge and reconnect a
      // fresh ws://127.0.0.1:9998 client. Re-dispatch per connection so the
      // second live socket is not starved by an earlier global "already sent".
      await maybeDispatchLocalProxyMessages(
        page,
        capture,
        localWebSocketServer,
        localProxyMessages,
        localProxyDelayMs,
        "loop",
        persistCapture,
      );
      lastLocalProxyRedispatchAt = Date.now();
    }
    if (Date.now() - lastRemixApplyAt >= 5_000) {
      if (await bestEffortDismissAistudioOverlays(page, capture)) {
        await persistCapture();
      }
      if (await bestEffortApplyRemixModal(page, capture)) {
        await persistCapture();
      }
      lastRemixApplyAt = Date.now();
    }
    if (
      shouldRetryAutoPromptDuringPolling({
        normalizedAutoPrompt,
        autoPromptSubmitted,
        nowMs: Date.now(),
        lastAutoPromptAttemptAtMs: lastAutoPromptAttemptAt,
      })
    ) {
      autoPromptSubmitted = await bestEffortAutoPrompt(
        page,
        normalizedAutoPrompt,
        capture,
        {
          maxAttempts: 1,
          pollMs: 0,
        },
      );
      lastAutoPromptAttemptAt = Date.now();
      await persistCapture();
    }
    await page.waitForTimeout(500);
  }
}

export { pollProbeCapture };
