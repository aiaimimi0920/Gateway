import { stdin, stdout } from "node:process";

async function readStdinJson() {
  const chunks = [];
  for await (const chunk of stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  const raw = Buffer.concat(chunks).toString("utf8").trim();
  if (!raw) {
    throw new Error("missing stdin JSON");
  }
  return JSON.parse(raw);
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function bodyIndicatesBrowserChallenge(text) {
  const lowered = text.toLowerCase();
  return (
    lowered.includes("google.com/sorry") ||
    lowered.includes("systems have detected unusual traffic")
  );
}

function bodyIndicatesCreateBlocked(text) {
  const lowered = text.toLowerCase();
  return (
    lowered.includes("can't create it right now") ||
    lowered.includes("can't seem to create any") ||
    lowered.includes("are you signed in") ||
    text.includes("您登录了吗") ||
    text.includes("似乎无法为您创建任何图片") ||
    text.includes("所在的地区尚未开通图片创建功能")
  );
}

function bodyIncludesImageArtifact(text) {
  return (
    text.includes("image_generation_content") &&
    /(googleusercontent|gstatic|blob:|data:image\/)/i.test(text)
  );
}

function bodyIndicatesImageEditAsyncFollowupReady(text) {
  if (!text.includes('"18"') || !text.includes('"21"')) {
    return false;
  }
  return /"18"\s*:\s*"r_[^"]+"/.test(text) && /"21"\s*:\s*\[[^\]]+\]/.test(text);
}

function bodyLooksLikeImageShortAck(text) {
  return (
    text.includes('["wrb.fr",null,null,null,null,[13]]') &&
    text.includes('"af.httprm"')
  );
}

function bodyContainsVideoPlaceholder(text) {
  return text.includes("video_placeholder");
}

function bodyContainsVideoBusyMessage(text) {
  return text.includes(
    "I couldn't do that because I'm getting a lot of requests right now. Please try again later.",
  );
}

async function collectResponseBody(response, timeoutMs, operation = null) {
  const body = response.body;
  if (!body) {
    return "";
  }

  const reader = body.getReader();
  const decoder = new TextDecoder();
  let text = "";
  let sawImageMarker = false;
  let idleBudgetMs = Math.min(timeoutMs, 60000);

  while (true) {
    const result = await Promise.race([
      reader.read(),
      sleep(idleBudgetMs).then(() => ({ idle: true })),
    ]);
    if (result && result.idle) {
      if (!text.trim()) {
        throw new Error("stream-read-idle-timeout");
      }
      break;
    }
    const { value, done } = result;
    if (done) {
      break;
    }
    text += decoder.decode(value, { stream: true });
    if (operation === "video") {
      if (bodyContainsVideoBusyMessage(text)) {
        break;
      }
      if (bodyContainsVideoPlaceholder(text)) {
        sawImageMarker = true;
        idleBudgetMs = Math.min(timeoutMs, 20000);
      }
    }
    if (bodyIndicatesBrowserChallenge(text) || bodyIndicatesCreateBlocked(text)) {
      break;
    }
    if (bodyLooksLikeImageShortAck(text)) {
      idleBudgetMs = Math.min(timeoutMs, 30000);
    }
    if (bodyIndicatesImageEditAsyncFollowupReady(text)) {
      idleBudgetMs = Math.min(timeoutMs, 45000);
    }
    if (text.includes("image_generation_content")) {
      sawImageMarker = true;
      idleBudgetMs = Math.min(timeoutMs, 15000);
    }
    if (bodyIncludesImageArtifact(text)) {
      await sleep(2000);
      break;
    }
    if (text.length >= 2_000_000) {
      break;
    }
  }

  await reader.cancel().catch(() => undefined);
  return text;
}

async function main() {
  const input = await readStdinJson();
  const url = String(input.url || "").trim();
  const queryPairs = Array.isArray(input.query) ? input.query : [];
  const rawPostData = String(input.rawPostData || "").trim();
  const cookieHeader = String(input.cookieHeader || "").trim();
  const operation = String(input.operation || "").trim().toLowerCase() || null;
  const timeoutMs = Math.max(1, Number(input.timeoutMs || 120000));
  const headers = { ...(input.headers || {}) };

  if (!url) {
    throw new Error("missing url");
  }
  if (!rawPostData) {
    throw new Error("missing rawPostData");
  }
  if (!cookieHeader) {
    throw new Error("missing cookieHeader");
  }

  const requestUrl = new URL(url);
  if (queryPairs.length) {
    requestUrl.search = new URLSearchParams(
      queryPairs.map((entry) => [String(entry?.[0] ?? ""), String(entry?.[1] ?? "")]),
    ).toString();
  }

  delete headers.cookie;
  delete headers.Cookie;
  headers.cookie = cookieHeader;

  const controller = new AbortController();
  const timer = setTimeout(
    () => controller.abort(new Error("gemini_canvas_http_replay_worker_timeout")),
    timeoutMs,
  );

  try {
    const response = await fetch(requestUrl, {
      method: "POST",
      headers,
      body: rawPostData,
      redirect: "follow",
      signal: controller.signal,
    });
    const contentType = response.headers.get("content-type");
    const bodyText = await collectResponseBody(response, timeoutMs, operation);
    stdout.write(
      `${JSON.stringify({
        ok: true,
        status: response.status,
        finalUrl: response.url,
        contentType,
        bodyText,
      })}\n`,
    );
  } finally {
    clearTimeout(timer);
  }
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  stdout.write(
    `${JSON.stringify({
      ok: false,
      error: {
        code: "gemini_canvas_http_replay_worker_failed",
        message,
        status: 500,
      },
    })}\n`,
  );
  process.exit(1);
});
