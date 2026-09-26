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
  const eventsController = new AbortController();
  const actionController = new AbortController();
  const signatureController = new AbortController();
  const timeoutId = setTimeout(() => {
    eventsController.abort("timeout");
    actionController.abort("timeout");
    signatureController.abort("timeout");
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
      ["finalizing", "completed"].includes(String(artifact.state ?? "").toLowerCase())
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

    return current ?? actionIds[0] ?? null;
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

    try {
      const menusResponse = await fetch(menusUrl, {
        method: "GET",
        credentials: "include",
        headers: {
          accept: "application/json, text/plain, */*",
          "x-client-capabilities": clientCapabilities,
          "x-client-context": clientContext,
        },
        signal: actionController.signal,
      });
      const menusText = await menusResponse.text().catch(() => "");
      if (!menusResponse.ok || !menusText) {
        return {
          actionBody: nextActionBody,
          resolvedActionType: currentActionType,
        };
      }

      const menusJson = JSON.parse(menusText);
      const discoveredActionType = selectActionTypeFromMenus(
        menusJson,
        operation,
        currentActionType,
      );
      if (discoveredActionType && discoveredActionType !== currentActionType) {
        nextActionBody.type = discoveredActionType;
        return {
          actionBody: nextActionBody,
          resolvedActionType: discoveredActionType,
        };
      }
    } catch {
      // Auto discovery is best-effort only; keep the caller/platform default.
    }

    return {
      actionBody: nextActionBody,
      resolvedActionType: currentActionType,
    };
  };

  const extractSignedUrl = (outputId, payload, signature) => {
    const frames = payload.split(/\n\n+/);
    for (const frame of frames) {
      const lines = frame.split("\n");
      for (const line of lines) {
        if (!line.startsWith("data:")) {
          continue;
        }
        const dataText = line.slice(5).trim();
        if (!dataText) {
          continue;
        }
        try {
          const parsed = JSON.parse(dataText);
          const objectRef = findCompletedArtifactObjectRef(outputId, parsed);
          if (objectRef) {
            return buildSignedUrlFromObjectRef(objectRef, signature);
          }
        } catch {
          // Ignore non-JSON SSE frames such as `data:` keepalive payloads.
        }
      }
    }
    return null;
  };

  try {
    const signatureResponse = await fetch(signatureUrl, {
      method: "GET",
      credentials: "include",
      headers: {
        accept: "application/json, text/plain, */*",
        "x-client-capabilities": clientCapabilities,
        "x-client-context": clientContext,
      },
      signal: signatureController.signal,
    });
    const signatureText = await signatureResponse.text().catch(() => "");
    if (!signatureResponse.ok) {
      return {
        ok: false,
        error: {
          code: "lumalabs_signature_failed",
          message: "LumaLabs signature request failed.",
          stage: "signature",
          status: signatureResponse.status,
          body: signatureText,
        },
      };
    }

    let signatureJson;
    try {
      signatureJson = JSON.parse(signatureText);
    } catch {
      return {
        ok: false,
        error: {
          code: "lumalabs_invalid_signature_response",
          message: "LumaLabs signature response was not valid JSON.",
          stage: "signature",
          status: signatureResponse.status,
          body: signatureText,
        },
      };
    }

    const wsToken =
      typeof signatureJson?.ws_token === "string" && signatureJson.ws_token.trim()
        ? signatureJson.ws_token.trim()
        : null;
    if (!wsToken) {
      return {
        ok: false,
        error: {
          code: "lumalabs_missing_ws_token",
          message: "LumaLabs signature response did not include ws_token.",
          stage: "signature",
          status: signatureResponse.status,
          body: signatureText,
        },
      };
    }

    const eventsResponse = await fetch(eventUrl, {
      method: "GET",
      credentials: "include",
      headers: {
        accept: "text/event-stream",
        authorization: `Bearer ${wsToken}`,
      },
      signal: eventsController.signal,
    });
    if (!eventsResponse.ok || !eventsResponse.body) {
      const bodyText = await eventsResponse.text().catch(() => "");
      return {
        ok: false,
        error: {
          code: "lumalabs_events_stream_failed",
          message: "LumaLabs events stream request failed.",
          stage: "events",
          status: eventsResponse.status,
          body: bodyText,
        },
      };
    }

    const resolvedAction = await resolveActionBody();
    const actionResponse = await fetch(actionUrl, {
      method: "POST",
      credentials: "include",
      headers: {
        accept: "application/json, text/plain, */*",
        "content-type": "application/json",
        "x-client-capabilities": clientCapabilities,
        "x-client-context": clientContext,
      },
      body: JSON.stringify(resolvedAction.actionBody),
      signal: actionController.signal,
    });
    const actionText = await actionResponse.text();
    if (!actionResponse.ok) {
      return {
        ok: false,
        error: {
          code: "lumalabs_action_failed",
          message: "LumaLabs action request failed.",
          stage: "action",
          status: actionResponse.status,
          body: actionText,
          resolvedActionType: resolvedAction.resolvedActionType,
        },
      };
    }

    let actionJson;
    try {
      actionJson = JSON.parse(actionText);
    } catch {
      return {
        ok: false,
        error: {
          code: "lumalabs_invalid_action_response",
          message: "LumaLabs action response was not valid JSON.",
          stage: "action",
          status: actionResponse.status,
          body: actionText,
        },
      };
    }

    const outputId = actionJson?.output_artifacts?.[artifactField]?.[0];
    if (!outputId || typeof outputId !== "string") {
      return {
        ok: false,
        error: {
          code: "lumalabs_missing_output_id",
          message: `LumaLabs action response did not include output_artifacts.${artifactField}[0].`,
          stage: "action",
          status: actionResponse.status,
          body: actionText,
        },
      };
    }

    const reader = eventsResponse.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";

    while (true) {
      const { done, value } = await reader.read();
      if (done) {
        break;
      }
      buffer += decoder.decode(value, { stream: true });
      const signedUrl = extractSignedUrl(outputId, buffer, signatureJson);
      if (signedUrl) {
        try {
          await reader.cancel();
        } catch {
          // Ignore cancellation errors on a finished SSE stream.
        }
        return {
          ok: true,
          signedUrl,
          outputId,
          resolvedActionType: resolvedAction.resolvedActionType,
          actionId:
            typeof actionJson?.action?.id === "string" && actionJson.action.id.trim()
              ? actionJson.action.id.trim()
              : null,
        };
      }
      if (buffer.length > 2_000_000) {
        buffer = buffer.slice(-250_000);
      }
    }

    return {
      ok: false,
      error: {
        code: "lumalabs_missing_signed_url",
        message: `LumaLabs events stream closed before output ${outputId} produced a signed URL.`,
        stage: "events_wait",
        status: 500,
      },
    };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const aborted =
      message.includes("aborted") || message.includes("timeout") || message.includes("AbortError");
    return {
      ok: false,
      error: {
        code: aborted ? "lumalabs_browser_timeout" : "lumalabs_browser_fetch_failed",
        message,
        stage: "browser_fetch",
        status: aborted ? 504 : 500,
      },
    };
  } finally {
    clearTimeout(timeoutId);
  }
}
