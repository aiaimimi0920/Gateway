import assert from "node:assert/strict";
import test from "node:test";

process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN = "1";

const {
  findAttachedGeminiAppPage,
  inspectContextEntryForReuse,
  pageLooksLikeReusableGeminiAppSurface,
  shouldAttemptConnectedClientFetchFallback,
} = await import("../gemini-canvas-browser-pool.mjs");

function openPage() {
  return {
    isClosed() {
      return false;
    },
  };
}

function closedPage() {
  return {
    isClosed() {
      return true;
    },
  };
}

function makePage(url, bodyText, { closed = false } = {}) {
  return {
    broughtToFront: false,
    url() {
      return url;
    },
    isClosed() {
      return closed;
    },
    async evaluate(fn) {
      return bodyText;
    },
    async bringToFront() {
      this.broughtToFront = true;
    },
  };
}

test("requests recreation when attached CDP browser is disconnected", () => {
  const entry = {
    attachedCdp: true,
    browser: {
      isConnected() {
        return false;
      },
    },
    context: {
      pages() {
        return [openPage()];
      },
    },
    page: openPage(),
  };

  assert.deepEqual(inspectContextEntryForReuse(entry), {
    recreate: true,
    adoptedPage: false,
  });
});

test("reuses entry and adopts a surviving page when cached page is closed", () => {
  const survivingPage = openPage();
  const entry = {
    attachedCdp: true,
    browser: {
      isConnected() {
        return true;
      },
    },
    context: {
      pages() {
        return [survivingPage];
      },
    },
    page: closedPage(),
  };

  assert.deepEqual(inspectContextEntryForReuse(entry), {
    recreate: false,
    adoptedPage: true,
  });
  assert.equal(entry.page, survivingPage);
});

test("findAttachedGeminiAppPage prefers an attached Gemini app surface page", async () => {
  const stalePage = makePage("https://example.com/", "not gemini");
  const geminiPage = makePage("https://gemini.google.com/app", "与 Gemini 对话\n制作图片\n快速");
  const entry = {
    attachedCdp: true,
    context: {
      pages() {
        return [stalePage, geminiPage];
      },
    },
  };

  const page = await findAttachedGeminiAppPage(entry, "https://gemini.google.com");
  assert.equal(page, geminiPage);
});

test("findAttachedGeminiAppPage ignores signed-out Gemini landing pages", async () => {
  const landingPage = makePage(
    "https://gemini.google.com/app",
    "登录\n认识 Gemini：你的私人 AI 助理",
  );
  const entry = {
    attachedCdp: true,
    context: {
      pages() {
        return [landingPage];
      },
    },
  };

  const page = await findAttachedGeminiAppPage(entry, "https://gemini.google.com");
  assert.equal(page, null);
});

test("pageLooksLikeReusableGeminiAppSurface accepts attached concrete app pages", () => {
  assert.equal(
    pageLooksLikeReusableGeminiAppSurface(
      "https://gemini.google.com/app/08f273300a7c4731",
      "Gemini\n与 Gemini 对话\n制作图片\n快速",
      "https://gemini.google.com",
    ),
    true,
  );
});

test("pageLooksLikeReusableGeminiAppSurface rejects signed-out landings on /app", () => {
  assert.equal(
    pageLooksLikeReusableGeminiAppSurface(
      "https://gemini.google.com/app",
      "登录\n认识 Gemini：你的私人 AI 助理",
      "https://gemini.google.com",
    ),
    false,
  );
});

test("requests recreation when context page enumeration throws", () => {
  const entry = {
    attachedCdp: false,
    browser: null,
    context: {
      pages() {
        throw new Error("context is already closed");
      },
    },
    page: closedPage(),
  };

  assert.deepEqual(inspectContextEntryForReuse(entry), {
    recreate: true,
    adoptedPage: false,
  });
});

test("retries POST browser fetch failures through connected client fallback", () => {
  assert.equal(
    shouldAttemptConnectedClientFetchFallback(new Error("page.evaluate: TypeError: Failed to fetch"), {
      method: "POST",
      useCanvasProxyMode: false,
    }),
    true,
  );
});

test("does not retry GET browser fetch failures through connected client fallback", () => {
  assert.equal(
    shouldAttemptConnectedClientFetchFallback(new Error("page.evaluate: TypeError: Failed to fetch"), {
      method: "GET",
      useCanvasProxyMode: false,
    }),
    false,
  );
});

test("does not retry page.evaluate fetch failures when already using canvas proxy mode", () => {
  assert.equal(
    shouldAttemptConnectedClientFetchFallback(new Error("page.evaluate: TypeError: Failed to fetch"), {
      method: "POST",
      useCanvasProxyMode: true,
    }),
    false,
  );
});
