import assert from "node:assert/strict";
import test from "node:test";
import { importScript, importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
test("image mode selection does not report success without any selection evidence", async () => {
  const { clickOperationMode } = await importTestableScript();

  const candidate = {
    first() {
      return this;
    },
    filter() {
      return this;
    },
    async waitFor() {},
    async click() {},
    async isVisible() {
      return false;
    },
  };

  const page = {
    getByRole() {
      return candidate;
    },
    getByText() {
      return candidate;
    },
    locator() {
      return candidate;
    },
    async evaluate() {
      return "Gemini\nFlash-Lite\nMeet Gemini, your personal AI assistant";
    },
    async waitForTimeout() {},
  };

  const selected = await clickOperationMode(page, "image", 1_000);

  assert.equal(selected, false);
});

test("ensureProgramPage falls back after domcontentloaded navigation timeout", async () => {
  const { ensureProgramPage } = await importTestableScript();
  const state = {
    currentUrl: "https://gemini.google.com/app",
    waitForLoadStateCalls: 0,
  };

  const page = {
    isClosed() {
      return false;
    },
    url() {
      return state.currentUrl;
    },
    async goto(url) {
      state.currentUrl = url;
      const error = new Error("page.goto: Timeout 25000ms exceeded.");
      error.name = "TimeoutError";
      throw error;
    },
    async waitForLoadState(kind) {
      state.waitForLoadStateCalls += 1;
      assert.equal(kind, "commit");
    },
    async waitForTimeout() {},
  };

  const navigated = await ensureProgramPage(
    { page },
    "https://gemini.google.com",
    "https://gemini.google.com/app/4abc4e7577b6149f",
    300_000,
  );

  assert.equal(navigated, true);
  assert.equal(state.currentUrl, "https://gemini.google.com/app/4abc4e7577b6149f");
  assert.equal(state.waitForLoadStateCalls, 1);
});

test("ensureProgramPage still surfaces non-timeout navigation failures", async () => {
  const { ensureProgramPage } = await importTestableScript();
  const page = {
    isClosed() {
      return false;
    },
    url() {
      return "https://gemini.google.com/app";
    },
    async goto() {
      throw new Error("navigation exploded");
    },
    async waitForLoadState() {},
    async waitForTimeout() {},
  };

  await assert.rejects(
    ensureProgramPage(
      { page },
      "https://gemini.google.com",
      "https://gemini.google.com/app/4abc4e7577b6149f",
      300_000,
    ),
    /navigation exploded/,
  );
});

test("ensureSharePage retries one transient connection-closed navigation failure", async () => {
  const { ensureSharePage } = await importTestableScript();
  let gotoCalls = 0;
  let currentUrl = "about:blank";
  const page = {
    isClosed() {
      return false;
    },
    url() {
      return currentUrl;
    },
    async goto(url) {
      gotoCalls += 1;
      if (gotoCalls === 1) {
        throw new Error("page.goto: net::ERR_CONNECTION_CLOSED");
      }
      currentUrl = url;
    },
    async waitForTimeout() {},
    async evaluate() {
      return "Try Gemini Canvas";
    },
  };

  await ensureSharePage({ page }, "https://gemini.google.com", "fe24c455a570", 2_000);

  assert.equal(gotoCalls, 2);
});

test("image mode selection falls back to the Images nav surface when create-image controls are absent", async () => {
  const { clickOperationMode } = await importTestableScript();

  const state = {
    currentUrl: "https://gemini.google.com/app",
    bodyText: "Conversation with Gemini",
  };

  function makeCandidate(kind, exists, onClick) {
    return {
      first() {
        return this;
      },
      filter() {
        return this;
      },
      async waitFor() {
        if (!exists) {
          throw new Error(`${kind}:missing`);
        }
      },
      async click() {
        if (!exists) {
          throw new Error(`${kind}:missing`);
        }
        onClick?.();
      },
      async isVisible() {
        return Boolean(exists);
      },
      async count() {
        return exists ? 1 : 0;
      },
    };
  }

  const missing = makeCandidate("missing", false);
  const imagesNav = makeCandidate("images_nav", true, () => {
    state.currentUrl = "https://gemini.google.com/images";
    state.bodyText = "Images";
  });

  const page = {
    getByRole(role, options = {}) {
      const name = String(options?.name ?? "");
      if (role === "link" && /Images/i.test(name)) {
        return imagesNav;
      }
      return missing;
    },
    getByText() {
      return missing;
    },
    locator(selector = "") {
      if (String(selector).includes('[aria-label*="Images"]')) {
        return imagesNav;
      }
      return missing;
    },
    async evaluate() {
      return state.bodyText;
    },
    url() {
      return state.currentUrl;
    },
    async waitForTimeout() {},
  };

  const selected = await clickOperationMode(page, "image", 2_000);

  assert.equal(selected, true);
  assert.equal(state.currentUrl, "https://gemini.google.com/images");
});

test("image media mode treats canvas proxy program pages as incompatible surfaces", async () => {
  const { shouldExitCanvasProgramSurfaceForMediaMode } = await importTestableScript();

  assert.equal(
    shouldExitCanvasProgramSurfaceForMediaMode("image", {
      bodyText: "Browser API Proxy Client",
      buttons: [{ text: "Try again without Canvas", ariaLabel: null, title: null }],
    }),
    true,
  );

  assert.equal(
    shouldExitCanvasProgramSurfaceForMediaMode("image", {
      bodyText: "Conversation with Gemini",
      buttons: [{ text: "Images", ariaLabel: "Images", title: null }],
    }),
    false,
  );
});

test("submitPrompt clicks Submit on image-generation surfaces when Send is absent", async () => {
  const { submitPrompt } = await importTestableScript();
  let submitClicked = false;

  const textbox = {
    first() {
      return this;
    },
    async waitFor() {},
    async focus() {},
    async evaluate() {
      return true;
    },
  };

  const missingCandidate = {
    first() {
      return this;
    },
    last() {
      return this;
    },
    async waitFor() {
      throw new Error("missing");
    },
    async click() {
      throw new Error("missing");
    },
  };

  const submitCandidate = {
    first() {
      return this;
    },
    last() {
      return this;
    },
    async waitFor() {},
    async click() {
      submitClicked = true;
    },
  };

  const page = {
    locator(selector = "") {
      const selectorText = String(selector);
      if (selectorText.includes('[role="textbox"]')) {
        return textbox;
      }
      if (
        selectorText.includes('button[aria-label*="Submit"]')
        || selectorText.includes('button[title*="Submit"]')
      ) {
        return submitCandidate;
      }
      return missingCandidate;
    },
    getByRole(role, options = {}) {
      if (role === "button" && /Submit/i.test(String(options?.name ?? ""))) {
        return submitCandidate;
      }
      return missingCandidate;
    },
    keyboard: {
      async press() {},
      async type() {},
    },
    async waitForTimeout() {},
  };

  await submitPrompt(page, "A small red cube on a white table", 2_000);

  assert.equal(submitClicked, true);
});

test("submitPrompt uses real keyboard typing for contenteditable Gemini composers", async () => {
  const { submitPrompt } = await importTestableScript();
  let typedPrompt = null;

  const textbox = {
    first() {
      return this;
    },
    async waitFor() {},
    async focus() {},
    async evaluate() {
      return {
        populated: true,
        editorKind: "contenteditable",
      };
    },
  };

  const submitCandidate = {
    first() {
      return this;
    },
    last() {
      return this;
    },
    async waitFor() {},
    async click() {},
  };

  const page = {
    locator(selector = "") {
      const selectorText = String(selector);
      if (selectorText.includes('[role="textbox"]')) {
        return textbox;
      }
      if (
        selectorText.includes('button[aria-label*="Submit"]')
        || selectorText.includes('button[title*="Submit"]')
      ) {
        return submitCandidate;
      }
      return submitCandidate;
    },
    getByRole() {
      return submitCandidate;
    },
    keyboard: {
      async press() {},
      async type(value) {
        typedPrompt = value;
      },
    },
    async waitForTimeout() {},
  };

  await submitPrompt(page, "typed through keyboard", 2_000);

  assert.equal(typedPrompt, "typed through keyboard");
});

test("image submission retry detector identifies template-only stalls on the /images composer", async () => {
  const { shouldRetryMediaPromptSubmission } = await importTestableScript();

  assert.equal(
    shouldRetryMediaPromptSubmission("image", {
      bodyText:
        "Create images\nTry a template or describe an idea in chat. Create with Nano Banana 2.\n\nA small red cube on a white table\n\nRequested aspect ratio: 1:1.\n\nImages\nPro\nMug\nBronze\nSubmit",
      events: [
        { type: "response", url: "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=MyzX6c&source-path=%2Fimages" },
        { type: "response", url: "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=V8rlHe&source-path=%2Fimages" },
        { type: "response", url: "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=XhaU0b&source-path=%2Fimages" },
      ],
      prompt: "A small red cube on a white table\n\nRequested aspect ratio: 1:1.",
    }),
    true,
  );

  assert.equal(
    shouldRetryMediaPromptSubmission("image", {
      bodyText: "Conversation with Gemini\nYou said\n\nA small red cube on a white table",
      events: [
        { type: "request", url: "https://gemini.google.com/_/BardChatUi/data/batchexecute?rpcids=MaZiqc&source-path=%2Fimages" },
      ],
      prompt: "A small red cube on a white table",
    }),
    false,
  );
});

test("video submission retry detector identifies an idle template surface", async () => {
  const { shouldRetryMediaPromptSubmission } = await importTestableScript();

  assert.equal(
    shouldRetryMediaPromptSubmission("video", {
      bodyText: "Create videos\nTry a template or describe a video in chat.\nTiny world\nAnime",
      prompt: "A paper airplane in a library",
    }),
    true,
  );
  assert.equal(
    shouldRetryMediaPromptSubmission("video", {
      bodyText: "Conversation with Gemini\nYou said\nA paper airplane in a library",
      prompt: "A paper airplane in a library",
    }),
    false,
  );
});

test("video provider gate recognizes the current English quota surface", async () => {
  const { detectMediaProviderGate } = await importTestableScript();

  assert.deepEqual(
    detectMediaProviderGate(
      "video",
      "You're out of videos for now. Videos will be available again on Aug 10 at 7:20 PM.",
    ),
    {
      status: 429,
      code: "gemini_canvas_video_quota_reached",
      message: "Gemini Canvas video generation quota is currently exhausted for this account.",
    },
  );
});

test("video player state recognizes the English ready surface", async () => {
  const { inferInvokeUiState } = await importTestableScript();

  assert.equal(
    inferInvokeUiState("video", {
      bodyText: "Your video is ready!\n0:00 / 0:10",
      buttons: [{ text: "", ariaLabel: "Download video", title: null }],
    }),
    "video_player_ready",
  );
});

test("video quota text does not hide an already completed result", async () => {
  const { detectMediaProviderGate, shouldBlockMediaProviderGate } = await importTestableScript();
  const bodyText = [
    "Your video is ready!",
    "You're out of videos for now",
    "Videos will be available again tomorrow.",
  ].join("\n");
  const providerGate = detectMediaProviderGate("video", bodyText);

  assert.equal(
    shouldBlockMediaProviderGate(
      "video",
      providerGate,
      [],
      { uiState: "video_player_ready" },
      bodyText,
    ),
    false,
  );
});
