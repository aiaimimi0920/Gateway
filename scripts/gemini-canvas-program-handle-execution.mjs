import { writeFileSync } from "node:fs";
import path from "node:path";
import {
  waitForShareSurface, hasPromptTextbox, tryFollowShareEntryPoint, clickNewChat,
  clickOperationMode, clickMediaActionButton, trySelectMusicStyleCard, submitPrompt,
} from "./gemini-canvas-program-handle-interaction.mjs";
import {
  mergeHints, hasCanvasProxyProgramCandidate, selectProgramConversationPair,
  selectLatestResponsePair, selectCanonicalProgramPair, concreteAppPathFromUrl,
  extractAppPath, deriveShareId, deriveConversationIdForAppPath,
} from "./gemini-canvas-program-handle-evidence.mjs";
import { mergeActionContract, extractCanvasProgramActionContractFromText } from "./gemini-canvas-browser-pool-action.mjs";
import { invokeContractIndicatesConcreteProgress } from "./gemini-canvas-browser-pool-media-policy.mjs";

export async function executeProgramHandleProbe({
  chromium, profileDir, executablePath, shareUrl, appUrl, operation, prompt,
  discoveryOnly, timeoutMs, outDir, collectSnapshot, startNetworkCapture,
  strongestHandle, hasTransportHints, toObjectKeyFromLocalPath, normalizeString,
  mergeInvokeContract, buildCanvasProgramInvokeContract,
}) {
  const context = await chromium.launchPersistentContext(profileDir, {
    executablePath,
    headless: false,
    locale: "zh-CN",
    args: ["--disable-dev-shm-usage", "--no-first-run", "--no-default-browser-check"],
  });

  let capture = null;
  try {
    const initialPage = context.pages()[0] ?? (await context.newPage());
    let activePage = initialPage;
    capture = startNetworkCapture(activePage);
    await capture.ready;
    const captureSnapshot = async (page, label) => {
      capture.assertHealthy();
      const snapshot = await collectSnapshot(page, label);
      capture.assertHealthy();
      return snapshot;
    };
    const aggregateHints = {
      appPaths: [],
      conversationIds: [],
      responseIds: [],
      sharePaths: [],
    };
    const adoptActivePage = async (nextPage) => {
      if (!nextPage || nextPage === activePage) {
        return;
      }
      await capture.adoptPage(nextPage);
      const previousPage = activePage;
      activePage = nextPage;
      if (previousPage && previousPage !== nextPage && !previousPage.isClosed()) {
        await previousPage.close().catch(() => undefined);
      }
    };

    await activePage.goto(shareUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await waitForShareSurface(activePage, Math.min(timeoutMs, 20000));
    await activePage.waitForTimeout(1500);
    const shareBefore = await captureSnapshot(activePage, "share-before");
    mergeHints(aggregateHints, shareBefore.handleHints);
    mergeActionContract(capture.state.actionContract, extractCanvasProgramActionContractFromText(shareBefore.bodyText));
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        capture.state.actionContract,
        capture.state.transportHints,
        shareBefore,
        prompt,
        capture.state,
      ),
    );

    let shareFollow = await tryFollowShareEntryPoint(activePage);
    if (shareFollow.page) {
      await adoptActivePage(shareFollow.page);
    }
    await activePage.waitForTimeout(3000);
    const shareAfter = await captureSnapshot(activePage, "share-after-cta");
    mergeHints(aggregateHints, shareAfter.handleHints);
    mergeActionContract(capture.state.actionContract, extractCanvasProgramActionContractFromText(shareAfter.bodyText));
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        capture.state.actionContract,
        capture.state.transportHints,
        shareAfter,
        prompt,
        capture.state,
      ),
    );

    if (activePage.url().includes("/share/") && !(await hasPromptTextbox(activePage))) {
      shareFollow = await tryFollowShareEntryPoint(activePage);
      if (shareFollow.page) {
        await adoptActivePage(shareFollow.page);
      }
      await activePage.waitForTimeout(3000);
    }

    if (!strongestHandle(aggregateHints)) {
      if (!discoveryOnly) {
        await activePage.goto(appUrl, {
          waitUntil: "domcontentloaded",
          timeout: timeoutMs,
        });
        await activePage.waitForTimeout(3000);
      }
    }

    const before = await captureSnapshot(activePage, "before");
    mergeHints(aggregateHints, before.handleHints);
    mergeActionContract(capture.state.actionContract, extractCanvasProgramActionContractFromText(before.bodyText));
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        capture.state.actionContract,
        capture.state.transportHints,
        before,
        prompt,
        capture.state,
      ),
    );

    if (!discoveryOnly && !(await hasPromptTextbox(activePage))) {
      await clickOperationMode(activePage, operation).catch(() => false);
      await activePage.waitForTimeout(2000);
    }

    const shouldStayOnProxyDiscoverySurface =
      discoveryOnly &&
      hasCanvasProxyProgramCandidate(capture.state.handlePairs, capture.state.invokeContract);
    const newChatClicked = shouldStayOnProxyDiscoverySurface ? false : await clickNewChat(activePage);
    const modeSelected = shouldStayOnProxyDiscoverySurface
      ? false
      : await clickOperationMode(activePage, operation);
    const afterNewChat = shouldStayOnProxyDiscoverySurface
      ? before
      : await captureSnapshot(activePage, "after-new-chat");
    mergeHints(aggregateHints, afterNewChat.handleHints);
    mergeActionContract(
      capture.state.actionContract,
      extractCanvasProgramActionContractFromText(afterNewChat.bodyText),
    );
    capture.state.invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        capture.state.actionContract,
        capture.state.transportHints,
        afterNewChat,
        prompt,
        capture.state,
      ),
    );

    if (
      !discoveryOnly &&
      operation === "music" &&
      /选择要混合制作的曲目/i.test(afterNewChat.bodyText)
    ) {
      const styleSelection = await trySelectMusicStyleCard(activePage, timeoutMs).catch(() => null);
      if (styleSelection?.clicked) {
        const afterStyleSelection = await captureSnapshot(activePage, "after-style-selection");
        mergeHints(aggregateHints, afterStyleSelection.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(afterStyleSelection.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            operation,
            capture.state.actionContract,
            capture.state.transportHints,
            afterStyleSelection,
            prompt,
            capture.state,
          ),
        );
      }
    }

    if (!discoveryOnly) {
      await submitPrompt(activePage, prompt);
    }

    const deadline = Date.now() + timeoutMs;
    let lastSnapshot = afterNewChat;
    let acceptedProgressReadyAt = null;
    while (Date.now() < deadline) {
      await activePage.waitForTimeout(1800);
      lastSnapshot = await captureSnapshot(activePage, "after");
      mergeHints(aggregateHints, lastSnapshot.handleHints);
      mergeHints(aggregateHints, capture.state.handleHints);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          capture.state.actionContract,
          capture.state.transportHints,
          lastSnapshot,
          prompt,
          capture.state,
        ),
      );
      const strongHandleReady = Boolean(strongestHandle(aggregateHints));
      const transportReady = hasTransportHints(capture.state.transportHints);
      const invokeReady = invokeContractIndicatesConcreteProgress(
        operation,
        capture.state.invokeContract,
        lastSnapshot,
      );
      if (invokeReady && !acceptedProgressReadyAt) {
        acceptedProgressReadyAt = Date.now();
      }
      if (
        discoveryOnly &&
        strongHandleReady &&
        (transportReady ||
          Boolean(capture.state.invokeContract?.transportKind) ||
          Boolean(capture.state.invokeContract?.actionName) ||
          Boolean(capture.state.invokeContract?.uiState))
      ) {
        break;
      }
      if (
        !discoveryOnly &&
        strongHandleReady &&
        (
          operation === "image" ||
          transportReady ||
          (
            invokeReady &&
            (
              operation !== "music" ||
              capture.state.invokeContract?.uiState === "music_player_ready" ||
              Date.now() - acceptedProgressReadyAt >= 12_000
            )
          )
        )
      ) {
        break;
      }
    }

    const stableProgramPair = selectProgramConversationPair(capture.state.handlePairs, null);
    const latestResponsePair = selectLatestResponsePair(capture.state.handlePairs);
    const actionContract = {
      canvasProgramAction:
        capture.state.actionContract.canvasProgramAction ??
        extractCanvasProgramActionContractFromText(lastSnapshot.bodyText).canvasProgramAction,
      canvasProgramActionInput:
        capture.state.actionContract.canvasProgramActionInput ??
        extractCanvasProgramActionContractFromText(lastSnapshot.bodyText).canvasProgramActionInput,
    };
    let invokeContract = mergeInvokeContract(
      capture.state.invokeContract,
      buildCanvasProgramInvokeContract(
        operation,
        actionContract,
        capture.state.transportHints,
        lastSnapshot,
        prompt,
        capture.state,
      ),
    );
    if (
      !discoveryOnly &&
      invokeContract &&
      !invokeContract.target &&
      (invokeContract.uiState === "music_player_ready" ||
        invokeContract.uiState === "video_player_ready")
    ) {
      const playClicked = await clickMediaActionButton(activePage, operation, "play", timeoutMs);
      if (playClicked) {
        await activePage.waitForTimeout(2000);
        lastSnapshot = await captureSnapshot(activePage, "after-play");
        mergeHints(aggregateHints, lastSnapshot.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            operation,
            capture.state.actionContract,
            capture.state.transportHints,
            lastSnapshot,
            prompt,
            capture.state,
          ),
        );
        invokeContract = capture.state.invokeContract;
      }
    }
    if (
      !discoveryOnly &&
      invokeContract &&
      !invokeContract.target &&
      (invokeContract.uiState === "music_player_ready" ||
        invokeContract.uiState === "video_player_ready")
    ) {
      const downloadClicked = await clickMediaActionButton(activePage, operation, "download", timeoutMs);
      if (downloadClicked) {
        await activePage.waitForTimeout(2500);
        lastSnapshot = await captureSnapshot(activePage, "after-download");
        mergeHints(aggregateHints, lastSnapshot.handleHints);
        mergeActionContract(
          capture.state.actionContract,
          extractCanvasProgramActionContractFromText(lastSnapshot.bodyText),
        );
        capture.state.invokeContract = mergeInvokeContract(
          capture.state.invokeContract,
          buildCanvasProgramInvokeContract(
            operation,
            capture.state.actionContract,
            capture.state.transportHints,
            lastSnapshot,
            prompt,
            capture.state,
          ),
        );
        invokeContract = capture.state.invokeContract;
      }
    }

    capture.assertHealthy();
    const canonicalProgramPair =
      selectCanonicalProgramPair(capture.state.handlePairs, lastSnapshot.url, null) ??
      stableProgramPair;
    const finalConcreteAppPath = concreteAppPathFromUrl(lastSnapshot.url);

    const result = {
      ok: true,
      profileDir,
      executablePath,
      outDir,
      shareUrl,
      appUrl,
      operation,
      prompt,
      discoveryOnly,
      shareFollowKind: shareFollow.kind,
      newChatClicked,
      modeSelected,
      beforeUrl: before.url,
      finalUrl: lastSnapshot.url,
      appPath:
        finalConcreteAppPath ??
        canonicalProgramPair?.appPath ??
        extractAppPath(lastSnapshot.url) ??
        [...aggregateHints.appPaths].reverse().find((value) =>
          /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value)),
        ) ??
        null,
      strongestHandle: strongestHandle(aggregateHints),
      aggregateHints,
      transportHints: capture.state.transportHints,
      candidatePairs: capture.state.handlePairs,
      stableProgramPair,
      latestResponsePair,
      canvasProgramAction: actionContract.canvasProgramAction,
      canvasProgramActionInput: actionContract.canvasProgramActionInput,
      canvasProgramInvokeContract: invokeContract,
      rpcCaptures: capture.state.rpcCaptures,
      networkSummary: {
        requestCount: capture.state.requests.length,
        responseCount: capture.state.responses.length,
        rpcCaptureCount: capture.state.rpcCaptures.length,
      },
      note: discoveryOnly
        ? "This probe stayed in discovery-only mode: it harvested the Canvas app contract without switching back into generic Gemini chat generation."
        : "A concrete /app/<id>, c_<id>, or stable share-derived entrypoint is a stronger program owner candidate than generic /app, but still does not by itself prove a separate Canvas quota lane.",
    };

    const normalizedProgramHandle = {
      ok: true,
      capturedAt: new Date().toISOString(),
      shareUrl,
      shareId: deriveShareId(shareUrl),
      beforeUrl: before.url,
      finalUrl: lastSnapshot.url,
      shareFollowKind: shareFollow.kind,
      newChatClicked,
      modeSelected,
      discoveryOnly,
      pageUrl: lastSnapshot.url,
      cookieHeader:
        normalizeString(invokeContract?.cookieHeader) ??
        normalizeString(capture.state.cookieHeader) ??
        null,
      programUrl:
        (finalConcreteAppPath ? `https://gemini.google.com${finalConcreteAppPath}` : null) ??
        canonicalProgramPair?.programUrl ??
        (result.appPath && /^\/app\//i.test(result.appPath)
          ? `https://gemini.google.com${result.appPath}`
          : null),
      appPath: finalConcreteAppPath ?? canonicalProgramPair?.appPath ?? result.appPath,
      conversationId:
        canonicalProgramPair?.conversationId ??
        deriveConversationIdForAppPath(result.appPath, aggregateHints.conversationIds),
      responseId: canonicalProgramPair?.responseId ?? null,
      invokeBaseUrl: [...(capture.state.transportHints.invokeBaseUrls || [])].reverse()[0] ?? null,
      musicWsUrl: [...(capture.state.transportHints.musicWsUrls || [])].reverse()[0] ?? null,
      videoInvokePath: [...(capture.state.transportHints.videoInvokePaths || [])].reverse()[0] ?? null,
      canvasProgramAction: actionContract.canvasProgramAction,
      canvasProgramActionInput: actionContract.canvasProgramActionInput,
      canvasProgramInvokeContract: invokeContract,
      rpcCaptures: capture.state.rpcCaptures,
      lastSeenConversationId: latestResponsePair?.conversationId ?? null,
      lastSeenResponseId:
        latestResponsePair?.responseId ?? [...aggregateHints.responseIds].reverse()[0] ?? null,
      candidatePairs: capture.state.handlePairs,
      aggregateHints,
      runtimeProfileDir: profileDir,
      runtimeStateObjectKey: toObjectKeyFromLocalPath(profileDir),
      sourceSummaryPath: path.join(outDir, "summary.json"),
      note: discoveryOnly
        ? "This handle was materialized in discovery-only mode. The browser stayed on Canvas app discovery and did not switch back into generic Gemini chat generation."
        : "This is a browser-probed Canvas program handle candidate. It is stronger than generic /app but still not proof of the final browserless quota lane.",
    };

    writeFileSync(
      path.join(outDir, "network-requests.json"),
      `${JSON.stringify(capture.state.requests, null, 2)}\n`,
      "utf8",
    );
    writeFileSync(
      path.join(outDir, "network-responses.json"),
      `${JSON.stringify(capture.state.responses, null, 2)}\n`,
      "utf8",
    );
    writeFileSync(
      path.join(outDir, "program-rpc-captures.json"),
      `${JSON.stringify(capture.state.rpcCaptures, null, 2)}\n`,
      "utf8",
    );
    writeFileSync(path.join(outDir, "summary.json"), `${JSON.stringify(result, null, 2)}\n`, "utf8");
    writeFileSync(
      path.join(outDir, "program-handle.json"),
      `${JSON.stringify(normalizedProgramHandle, null, 2)}\n`,
      "utf8",
    );
    console.log(JSON.stringify(result, null, 2));
  } finally {
    try {
      await capture?.stop();
    } catch {
      // Cleanup must not replace the probe outcome.
    }
    await context.close().catch(() => undefined);
  }
}
