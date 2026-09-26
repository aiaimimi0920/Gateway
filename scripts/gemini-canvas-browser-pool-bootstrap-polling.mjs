export function createBootstrapPollingOwner({
  collectProgramHandleSnapshot, buildProgramHandleState, hasConcreteProgramHandleState,
  invokeContractIndicatesConcreteProgress, hasTransportHints,
}) {
  async function pollBootstrapProgram({
    activePage, captureState, baseUrl, args, bootstrapOperation, discoveryOnly,
    timeoutMs, initialSnapshot, mergeSnapshot,
  }) {
    const deadline = Date.now() + timeoutMs;
    let lastSnapshot = initialSnapshot;
    let concreteHandleReadyAt = null;
    let acceptedProgressReadyAt = null;
    while (Date.now() < deadline) {
      await activePage.waitForTimeout(1800);
      lastSnapshot = await collectProgramHandleSnapshot(activePage);
      mergeSnapshot(lastSnapshot);
      const currentHandleState = buildProgramHandleState(
        baseUrl,
        args,
        lastSnapshot.url,
        captureState,
      );
      if (hasConcreteProgramHandleState(currentHandleState)) {
        if (!concreteHandleReadyAt) {
          concreteHandleReadyAt = Date.now();
        }
        const invokeReady =
          invokeContractIndicatesConcreteProgress(
            bootstrapOperation,
            captureState.invokeContract,
            lastSnapshot,
          );
        if (invokeReady && !acceptedProgressReadyAt) {
          acceptedProgressReadyAt = Date.now();
        }
        if (
          discoveryOnly &&
          (hasTransportHints(captureState.transportHints) ||
            Boolean(captureState.invokeContract?.transportKind) ||
            Boolean(captureState.invokeContract?.actionName) ||
            Boolean(captureState.invokeContract?.uiState))
        ) {
          break;
        }
        if (
          !discoveryOnly &&
          (
            bootstrapOperation === "text" ||
            bootstrapOperation === "image" ||
            hasTransportHints(captureState.transportHints) ||
            (
              invokeReady &&
              (
                bootstrapOperation !== "music" ||
                captureState.invokeContract?.uiState === "music_player_ready" ||
                Date.now() - acceptedProgressReadyAt >= 12_000
              )
            ) ||
            Date.now() - concreteHandleReadyAt >= 15_000
          )
        ) {
          break;
        }
      }
    }
    return lastSnapshot;
  }

  return { pollBootstrapProgram };
}
