// Playwright serializes this callback; it cannot capture Node module bindings.
export const pollProducerPageVideoStatus = async ({
  baseUrl,
  headers,
  jobId,
  timeoutMs,
}) => {
  const STATUS_POLL_INTERVAL_MS = 5000;
  const readResponseText = async (response, signal) => {
    if (signal.aborted) throw signal.reason;
    if (!response.body) return "";
    const reader = response.body.getReader();
    const limit = 16 * 1024 * 1024;
    let buffer = null;
    let bytes = 0;
    // Cancel explicitly so a pending read settles when the phase is aborted.
    const cancel = () => { void reader.cancel(signal.reason).catch(() => undefined); };
    signal.addEventListener("abort", cancel, { once: true });
    try {
      while (true) {
        if (signal.aborted) throw signal.reason;
        const { value, done } = await reader.read();
        if (signal.aborted) throw signal.reason;
        if (done) break;
        if (value.byteLength > limit - bytes) {
          throw Object.assign(new Error("Producer status response exceeds 16 MiB."), {
            code: "producer_browser_status_response_too_large",
          });
        }
        if (!value.byteLength) continue;
        const required = bytes + value.byteLength;
        if (!buffer || required > buffer.byteLength) {
          const capacity = Math.min(
            limit,
            Math.max(64 * 1024, required, (buffer?.byteLength ?? 0) * 2),
          );
          const grown = new Uint8Array(capacity);
          if (buffer) grown.set(buffer.subarray(0, bytes));
          buffer = grown;
        }
        buffer.set(value, bytes);
        bytes += value.byteLength;
      }
      return buffer ? new TextDecoder().decode(buffer.subarray(0, bytes)) : "";
    } finally {
      signal.removeEventListener("abort", cancel);
      await reader.cancel().catch(() => undefined);
      reader.releaseLock();
    }
  };
  const delay = (ms, signal) => new Promise((resolve, reject) => {
    let timerId;
    const finish = (error) => {
      clearTimeout(timerId);
      signal.removeEventListener("abort", onAbort);
      if (error) reject(error);
      else resolve();
    };
    const onAbort = () => finish(new Error("Status polling aborted."));
    if (signal.aborted) {
      onAbort();
      return;
    }
    signal.addEventListener("abort", onAbort, { once: true });
    timerId = setTimeout(() => finish(), ms);
  });

  const parseMaybeJson = (value) => {
    if (typeof value !== "string") {
      return value ?? null;
    }
    const trimmed = value.trim();
    if (!trimmed) {
      return null;
    }
    try {
      return JSON.parse(trimmed);
    } catch {
      return trimmed;
    }
  };

  const collectUrls = (value, urls = []) => {
    const fail = () => {
      throw Object.assign(new Error("Producer media payload exceeds traversal limits."), {
        status: 502,
        code: "producer_media_payload_too_complex",
      });
    };
    if (urls.length > 1024) fail();
    const stack = [{ value, depth: 0 }];
    const active = new WeakSet();
    let scheduled = 1;
    while (stack.length) {
      const item = stack.pop();
      const current = item.value;
      if (item.exit) {
        active.delete(current);
        continue;
      }
      if (item.depth > 64) fail();
      if (typeof current === "string") {
        if (/^https?:\/\//i.test(current) && /\.(mp4|mov)(\?|$)/i.test(current)) {
          if (urls.length >= 1024) fail();
          urls.push(current);
        }
        continue;
      }
      if (!current || typeof current !== "object") continue;
      if (active.has(current)) fail();
      const children = Array.isArray(current) ? current : [];
      if (!Array.isArray(current)) {
        for (const key in current) {
          if (!Object.prototype.hasOwnProperty.call(current, key)) continue;
          if (children.length >= 100_000 - scheduled) fail();
          children.push(current[key]);
        }
      }
      if (children.length > 100_000 - scheduled) fail();
      scheduled += children.length;
      active.add(current);
      stack.push({ value: current, exit: true });
      // Reverse pushes preserve depth-first order; only ancestor cycles are rejected.
      for (let index = children.length - 1; index >= 0; index--) {
        stack.push({ value: children[index], depth: item.depth + 1 });
      }
    }
    return urls;
  };

  const chooseVideoUrl = (urls, jobId) => {
    if (!Array.isArray(urls) || urls.length === 0) {
      return null;
    }
    return (
      urls.find((entry) => entry.includes(`/music-video/${jobId}/`)) ??
      urls.find((entry) => entry.includes("/music-video/")) ??
      urls[0]
    );
  };

  const pollVideoStatus = async ({ baseUrl, headers, jobId, timeoutMs }) => {
    const deadline = Date.now() + timeoutMs;
    let latestPayload = null;

    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
    const timeoutResult = () => ({
      ok: false,
      error: {
        status: 504,
        code: "producer_browser_video_timeout",
        message: "Timed out while waiting for Producer video generation to complete.",
        body: latestPayload ? JSON.stringify(latestPayload) : undefined,
      },
    });
    try {
      while (Date.now() < deadline) {
        const response = await fetch(`${baseUrl}/__api/music-video/${jobId}/status`, {
          method: "GET",
          credentials: "include",
          redirect: "error",
          signal: controller.signal,
          headers: {
            accept: "application/json, text/plain, */*",
            authorization: headers.authorization,
            origin: headers.origin,
            referer: headers.referer,
          },
        });
        const bodyText = await readResponseText(response, controller.signal);
        const parsedBody = parseMaybeJson(bodyText);
        if (!response.ok) {
          return {
            ok: false,
            error: {
              status: response.status,
              code: "producer_browser_status_failed",
              message: "Producer music-video status request failed.",
              body:
                typeof parsedBody === "string" ? parsedBody : JSON.stringify(parsedBody),
            },
          };
        }

        latestPayload = parsedBody;
        const status =
          parsedBody && typeof parsedBody === "object" && typeof parsedBody.status === "string"
            ? parsedBody.status.trim().toLowerCase()
            : null;
        const videoUrls = collectUrls(parsedBody, []);
        const videoUrl = chooseVideoUrl(videoUrls, jobId);
        const previewUrl =
          videoUrls.find((entry) => entry !== videoUrl) ??
          videoUrls.find((entry) => entry.includes("sample_")) ??
          null;

        if (status === "completed") {
          return {
            ok: true,
            result: {
              statusPayload: parsedBody,
              videoUrl,
              previewUrl,
            },
          };
        }

        if (["failed", "error", "cancelled", "canceled"].includes(status)) {
          return {
            ok: false,
            error: {
              status: 502,
              code: "producer_browser_video_failed",
              message: `Producer video job entered terminal status '${status}'.`,
              body: JSON.stringify(parsedBody),
            },
          };
        }

        await delay(STATUS_POLL_INTERVAL_MS, controller.signal);
      }
      return timeoutResult();
    } catch (error) {
      if (controller.signal.aborted) {
        return timeoutResult();
      }
      if (error?.code === "producer_media_payload_too_complex") {
        return {
          ok: false,
          error: {
            status: 502,
            code: "producer_media_payload_too_complex",
            message: "Producer media payload exceeds traversal limits.",
          },
        };
      }
      if (error?.code === "producer_browser_status_response_too_large") {
        return {
          ok: false,
          error: {
            status: 502,
            code: "producer_browser_status_response_too_large",
            message: "Producer status response exceeds 16 MiB.",
          },
        };
      }
      return {
        ok: false,
        error: {
          status: 502,
          code: "producer_browser_status_fetch_failed",
          message: "Producer music-video status transport failed.",
        },
      };
    } finally {
      clearTimeout(timeoutId);
    }
  };

  return await pollVideoStatus({ baseUrl, headers, jobId, timeoutMs });
};
