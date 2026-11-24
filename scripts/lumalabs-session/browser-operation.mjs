/** Browser-only Luma operation protocol; self-contained for page.evaluate serialization. */
export async function executeLumaOperation({
  realmId,
  mediaOperation,
  artifactField,
  locale,
  actionBody,
  autoDiscoverActionType,
  timeoutMs,
  clientCapabilities,
}) {
  const eventUrl = `${window.location.origin}/events/api/vespa/realms/${realmId}/events/stream/grip`;
  const actionUrl = `${window.location.origin}/api/vespa/realms/${realmId}/actions`;
  const menusUrl = `${window.location.origin}/api/vespa/realms/${realmId}/menus.json?v=1`;
  const signatureUrl = `${window.location.origin}/api/vespa/realms/${realmId}/signature`;
  const clientContext = `id=${crypto.randomUUID()},name=web,locale=${locale}`;
  const controller = new AbortController();
  let timedOut = false;
  let reader;
  let resourceLimit;
  const failure = (code, message, stage = "browser_fetch", status = 500, extra = {}) => ({
    ok: false, error: { code, message, stage, status, ...extra },
  });
  const timeoutId = setTimeout(() => {
    timedOut = true;
    controller.abort();
  }, timeoutMs);

  const buildSignedUrlFromObjectRef = (objectRef, signature) => {
    if (typeof objectRef !== "string" || !objectRef.trim()) {
      return null;
    }
    const cdnUrl =
      typeof signature?.cdn_url === "string" && signature.cdn_url.trim()
        ? signature.cdn_url.trim().replace(/\/+$/, "")
        : null;
    const queryParams =
      typeof signature?.query_params === "string" && signature.query_params.trim()
        ? signature.query_params.trim().replace(/^\?+/, "")
        : null;
    if (!cdnUrl || !queryParams) {
      return null;
    }
    return `${cdnUrl}/${objectRef.replace(/^\/+/, "")}?${queryParams}`;
  };

  const findCompletedArtifactObjectRef = (outputId, value) => {
    if (!value || typeof value !== "object") {
      return null;
    }
    if (Array.isArray(value)) {
      for (const entry of value) {
        const result = findCompletedArtifactObjectRef(outputId, entry);
        if (result) {
          return result;
        }
      }
      return null;
    }

    const artifact =
      value &&
      typeof value === "object" &&
      value.artifact &&
      typeof value.artifact === "object"
        ? value.artifact
        : null;
    if (
      artifact &&
      artifact.id === outputId &&
      typeof artifact.object_ref === "string" &&
      artifact.object_ref.trim() &&
      artifact.state === "completed"
    ) {
      return artifact.object_ref.trim();
    }

    for (const entry of Object.values(value)) {
      const result = findCompletedArtifactObjectRef(outputId, entry);
      if (result) {
        return result;
      }
    }
    return null;
  };

  const cloneActionBody = (value) => {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
      return {};
    }
    const next = { ...value };
    if (next.fields && typeof next.fields === "object" && !Array.isArray(next.fields)) {
      next.fields = { ...next.fields };
    }
    return next;
  };

  const collectActionItems = (value, bucket = []) => {
    if (!value || typeof value !== "object") {
      return bucket;
    }
    if (Array.isArray(value)) {
      for (const entry of value) {
        collectActionItems(entry, bucket);
      }
      return bucket;
    }
    if (value.type === "action" && typeof value.id === "string" && value.id.trim()) {
      bucket.push(value);
    }
    for (const entry of Object.values(value)) {
      collectActionItems(entry, bucket);
    }
    return bucket;
  };

  const actionDiscoveryPlanForOperation = (operation) => {
    switch ((operation || "").trim().toLowerCase()) {
      case "image":
        return {
          exactIds: ["create_image_uni_1"],
          prefixIds: ["create_image_"],
        };
      case "video":
        return {
          exactIds: ["create_video_ray3_14"],
          prefixIds: ["create_video_"],
        };
      case "audio":
        return {
          exactIds: ["text_to_music_elevenlabs_v1"],
          prefixIds: [
            "text_to_music_",
            "create_audio_",
            "text_to_sfx_",
            "text_to_speech_",
            "audio_",
          ],
        };
      default:
        return null;
    }
  };

  const selectActionTypeFromMenus = (menusPayload, operation, currentActionType) => {
    const plan = actionDiscoveryPlanForOperation(operation);
    if (!plan) {
      return typeof currentActionType === "string" ? currentActionType.trim() : null;
    }

    const items = collectActionItems(menusPayload?.menus ?? menusPayload);
    const actionIds = [...new Set(
      items
        .map((entry) =>
          typeof entry?.id === "string" && entry.id.trim() ? entry.id.trim() : null,
        )
        .filter(Boolean),
    )];
    if (actionIds.length === 0) {
      return typeof currentActionType === "string" ? currentActionType.trim() : null;
    }

    const current =
      typeof currentActionType === "string" && currentActionType.trim()
        ? currentActionType.trim()
        : null;
    if (current && actionIds.includes(current)) {
      return current;
    }

    for (const exactId of plan.exactIds) {
      if (actionIds.includes(exactId)) {
        return exactId;
      }
    }

    for (const prefix of plan.prefixIds) {
      const matched = actionIds.find((entry) =>
        entry.toLowerCase().startsWith(prefix.toLowerCase()),
      );
      if (matched) {
        return matched;
      }
    }

    // A populated menu cannot authorize an unrelated action or an absent default.
    return null;
  };

  // Metadata is untrusted decoded bytes, including optional menu discovery.
  const readMetadata = async (response) => {
    if (!response.body) return "";
    const source = response.body.getReader();
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let total = 0;
    let text = "";
    try {
      while (true) {
        const { done, value } = await source.read();
        if (done) return text + decoder.decode();
        total += value.byteLength;
        if (total > 128 * 1024) {
          resourceLimit = "lumalabs_browser_metadata_limit";
          throw new Error("Metadata budget exceeded.");
        }
        text += decoder.decode(value, { stream: true });
      }
    } finally {
      try { await source.cancel(); } finally { source.releaseLock(); }
    }
  };

  const resolveActionBody = async () => {
    const nextActionBody = cloneActionBody(actionBody);
    if (!autoDiscoverActionType) {
      return {
        actionBody: nextActionBody,
        resolvedActionType:
          typeof nextActionBody.type === "string" ? nextActionBody.type.trim() : null,
      };
    }

    const currentActionType =
      typeof nextActionBody.type === "string" ? nextActionBody.type.trim() : null;
    const operation = typeof mediaOperation === "string" ? mediaOperation.trim() : "";
    if (!currentActionType || !operation) {
      return {
        actionBody: nextActionBody,
        resolvedActionType: currentActionType,
      };
    }

    const original = { actionBody: nextActionBody, resolvedActionType: currentActionType };
    let menusResponse;
    try {
      menusResponse = await fetch(menusUrl, {
        method: "GET",
        credentials: "include",
        headers: {
          accept: "application/json, text/plain, */*",
          "x-client-capabilities": clientCapabilities,
          "x-client-context": clientContext,
        },
        signal: controller.signal,
      });
    } catch {
      return original; // Discovery transport remains best-effort, unlike reader safety.
    }
    if (!menusResponse.ok) return original;
    const menusText = await readMetadata(menusResponse);
    if (!menusText) return original;
    try {
      const menusJson = JSON.parse(menusText);
      const discoveredActionType = selectActionTypeFromMenus(
        menusJson,
        operation,
        currentActionType,
      );
      if (!discoveredActionType) return null;
      if (discoveredActionType && discoveredActionType !== currentActionType) {
        nextActionBody.type = discoveredActionType;
        return {
          actionBody: nextActionBody,
          resolvedActionType: discoveredActionType,
        };
      }
    } catch {
      // Invalid optional menu JSON keeps the caller/platform default.
    }
    return original;
  };

  const extractSignedUrl = (outputId, frame, signature) => {
    const data = frame.split("\n").filter((line) => line.startsWith("data:"))
      .map((line) => line.slice(5).replace(/^ /, "")).join("\n");
    if (!data) return null;
    let parsed;
    try { parsed = JSON.parse(data); }
    catch { return null; } // Ignore non-JSON keepalive frames, not partial events.
    const objectRef = findCompletedArtifactObjectRef(outputId, parsed);
    return objectRef ? buildSignedUrlFromObjectRef(objectRef, signature) : null;
  };

  try {
    const signatureResponse = await fetch(signatureUrl, {
      method: "GET", credentials: "include",
      headers: {
        accept: "application/json, text/plain, */*",
        "x-client-capabilities": clientCapabilities, "x-client-context": clientContext,
      },
      signal: controller.signal,
    });
    if (!signatureResponse.ok) {
      return failure("lumalabs_signature_failed", "LumaLabs signature request failed.", "signature", signatureResponse.status);
    }
    const signatureText = await readMetadata(signatureResponse);
    let signatureJson;
    try { signatureJson = JSON.parse(signatureText); }
    catch {
      return failure("lumalabs_invalid_signature_response", "LumaLabs signature response was not valid JSON.", "signature");
    }
    const wsToken = typeof signatureJson?.ws_token === "string" ? signatureJson.ws_token.trim() : "";
    if (!wsToken) {
      return failure("lumalabs_missing_ws_token", "LumaLabs signature response did not include ws_token.", "signature");
    }
    const eventsResponse = await fetch(eventUrl, {
      method: "GET", credentials: "include",
      headers: { accept: "text/event-stream", authorization: `Bearer ${wsToken}` },
      signal: controller.signal,
    });
    if (eventsResponse.body) reader = eventsResponse.body.getReader();
    if (!eventsResponse.ok || !reader ||
        eventsResponse.headers.get("content-type")?.split(";")[0].trim().toLowerCase() !== "text/event-stream") {
      return failure("lumalabs_events_stream_failed", "LumaLabs events stream request failed.", "events", eventsResponse.status);
    }
    const resolvedAction = await resolveActionBody();
    if (!resolvedAction) {
      return failure("lumalabs_action_unavailable", "LumaLabs menu did not offer an action for this operation.", "action_discovery", 502);
    }
    // Fetch must not follow redirects that can resend this billable POST.
    const actionResponse = await fetch(actionUrl, {
      method: "POST", credentials: "include", redirect: "manual",
      headers: {
        accept: "application/json, text/plain, */*", "content-type": "application/json",
        "x-client-capabilities": clientCapabilities, "x-client-context": clientContext,
      },
      body: JSON.stringify(resolvedAction.actionBody), signal: controller.signal,
    });
    if (actionResponse.type === "opaqueredirect") {
      return failure("lumalabs_action_redirect", "LumaLabs redirected the action; submission is unconfirmed and was not replayed.", "action", 502);
    }
    if (actionResponse.status === 402) {
      // Preserve the actionable refusal without reading private balance or payment details.
      return failure("lumalabs_action_payment_required",
        "LumaLabs declined the action: payment or additional credits are required.",
        "action", 402, { resolvedActionType: resolvedAction.resolvedActionType });
    }
    if (!actionResponse.ok) {
      return failure("lumalabs_action_failed", "LumaLabs action request failed.", "action", actionResponse.status,
        { resolvedActionType: resolvedAction.resolvedActionType });
    }
    const actionText = await readMetadata(actionResponse);
    let actionJson;
    try { actionJson = JSON.parse(actionText); }
    catch {
      return failure("lumalabs_invalid_action_response", "LumaLabs action response was not valid JSON.", "action");
    }
    const outputIds = actionJson?.output_artifacts?.[artifactField];
    if (!Array.isArray(outputIds) || outputIds.length !== 1 ||
        typeof outputIds[0] !== "string" || !outputIds[0].trim() || outputIds[0] !== outputIds[0].trim()) {
      return failure("lumalabs_missing_output_id", "LumaLabs action did not identify exactly one output.", "action");
    }
    const outputId = outputIds[0];
    const decoder = new TextDecoder("utf-8", { fatal: true });
    let buffer = "";
    let searchFrom = 0;
    let previousChunkEndedWithCr = false;
    let total = 0;
    while (true) {
      const { done, value } = await reader.read();
      if (done) { decoder.decode(); break; }
      total += value.byteLength;
      if (total > 2 * 1024 * 1024) {
        return failure("lumalabs_browser_events_limit", "LumaLabs events exceeded the cumulative byte budget.", "events_wait");
      }
      let chunk = decoder.decode(value, { stream: true });
      if (!chunk) continue;
      // A CR already terminates its line; a following LF is the rest of the same ending.
      if (previousChunkEndedWithCr && chunk.startsWith("\n")) chunk = chunk.slice(1);
      previousChunkEndedWithCr = chunk.endsWith("\r");
      buffer += chunk.replace(/\r\n?/g, "\n");
      let frameStart = 0;
      let boundary;
      while ((boundary = buffer.indexOf("\n\n", searchFrom)) !== -1) {
        const frame = buffer.slice(frameStart, boundary);
        frameStart = searchFrom = boundary + 2;
        const signedUrl = extractSignedUrl(outputId, frame, signatureJson);
        if (signedUrl) {
          return {
            ok: true, signedUrl, outputId, realmId, mediaOperation, artifactField, state: "completed",
            resolvedActionType: resolvedAction.resolvedActionType,
            actionId: typeof actionJson?.action?.id === "string" && actionJson.action.id.trim()
              ? actionJson.action.id.trim() : null,
          };
        }
      }
      buffer = buffer.slice(frameStart);
      // Only the last buffered character can join a delimiter in the next chunk.
      searchFrom = Math.max(0, buffer.length - 1);
    }
    return failure("lumalabs_missing_signed_url", "LumaLabs stream closed before the original output completed.", "events_wait");
  } catch {
    return failure(resourceLimit ?? (timedOut ? "lumalabs_browser_timeout" : "lumalabs_browser_fetch_failed"),
      timedOut ? "LumaLabs browser deadline expired." : "LumaLabs browser request failed; details omitted.",
      "browser_fetch", timedOut ? 504 : 500);
  } finally {
    // Cancel before abort: aborting a live Fetch reader first can itself reject cancel().
    let cleanupFailed = false;
    if (reader) {
      try { await reader.cancel(); } catch { cleanupFailed = true; }
      try { reader.releaseLock(); } catch { cleanupFailed = true; }
    }
    controller.abort();
    clearTimeout(timeoutId);
    if (timedOut) return failure("lumalabs_browser_timeout", "LumaLabs browser deadline expired.", "browser_fetch", 504);
    if (cleanupFailed) {
      return failure("lumalabs_events_cleanup_failed", "LumaLabs events cleanup failed.", "events");
    }
  }
}
