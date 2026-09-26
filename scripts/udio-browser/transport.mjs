import { isChallengeOutcome, trimBody } from "./responses.mjs";

export async function browserFetch(page, args) {
  return await page.evaluate(
    async ({ requestUrl, method, headers, bodyText, timeoutMs }) => {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
      try {
        const response = await fetch(requestUrl, {
          method,
          headers,
          body: typeof bodyText === "string" ? bodyText : undefined,
          credentials: "include",
          signal: controller.signal,
        });
        let text = "";
        if (response.body) {
          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          const block = new Uint8Array(64 * 1024);
          const segments = [];
          let bytes = 0;
          let used = 0;
          let complete = false;
          try {
            while (true) {
              const { value, done } = await reader.read();
              if (done) { complete = true; break; }
              bytes += value.byteLength;
              if (bytes > 16 * 1024 * 1024) {
                throw Object.assign(new Error("Udio browser JSON response exceeds 16 MiB."), { status: 502 });
              }
              // Fixed blocks bound retained metadata even for one-byte stream chunks.
              let offset = 0;
              while (offset < value.byteLength) {
                const count = Math.min(block.length - used, value.byteLength - offset);
                block.set(value.subarray(offset, offset + count), used);
                used += count;
                offset += count;
                if (used === block.length) {
                  segments.push(decoder.decode(block, { stream: true }));
                  used = 0;
                }
              }
            }
            segments.push(decoder.decode(block.subarray(0, used)));
            text = segments.join("");
          } finally {
            if (!complete) await reader.cancel().catch(() => undefined);
            reader.releaseLock();
          }
        }
        return {
          ok: response.ok,
          status: response.status,
          contentType: response.headers.get("content-type") || "",
          mitigated: response.headers.get("x-vercel-mitigated") || "",
          text,
        };
      } catch (error) {
        return {
          ok: false,
          status: error?.status === 502 ? 502 : 504,
          transportError: true,
          text: typeof error?.message === "string" ? error.message : String(error),
        };
      } finally {
        clearTimeout(timeoutId);
      }
    },
    {
      requestUrl: args.requestUrl,
      method: args.method,
      headers: args.headers ?? {},
      bodyText: args.bodyText ?? null,
      timeoutMs: args.timeoutMs,
    },
  );
}

export async function runWithChallengeRetries(args) {
  let challengeAttempted = false;
  let lastChallenge = null;
  while (Date.now() < args.overallDeadline) {
    const outcome = await args.action();
    if (outcome.transportError) {
      throw Object.assign(new Error(`Browser request failed: ${outcome.text}`), {
        status: outcome.status ?? 504,
        code: "udio_browser_transport_failed",
        body: outcome.text,
      });
    }
    if (!isChallengeOutcome(outcome)) {
      return outcome;
    }

    lastChallenge = outcome;
    if (!challengeAttempted) {
      challengeAttempted = true;
      await args.onChallenge();
    }

    const remainingMs = args.overallDeadline - Date.now();
    if (remainingMs <= 0) {
      break;
    }
    await new Promise((resolve) =>
      setTimeout(resolve, Math.min(args.retryIntervalMs, remainingMs)),
    );
  }

  throw Object.assign(
    new Error(
      "Udio requires a browser security check or captcha challenge before generation can continue.",
    ),
    {
      status: lastChallenge?.status ?? 429,
      code: "udio_browser_challenge_required",
      body: trimBody(lastChallenge?.text ?? ""),
    },
  );
}
