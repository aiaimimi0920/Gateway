import assert from "node:assert/strict";
import fs from "node:fs/promises";
import test from "node:test";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "gemini-canvas-browser-pool.mjs",
);

async function importScript() {
  process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN = "1";
  return await import(pathToFileURL(scriptPath).href);
}

async function importTestableScript() {
  process.env.GEMINI_CANVAS_BROWSER_POOL_SUPPRESS_MAIN = "1";
  const source = await fs.readFile(scriptPath, "utf8");
  const exportBlock = `
export {
  applyGeminiAccountScope,
  canvasProxyPreviewNeedsDirectLaunch,
  bodyContainsSubmittedPrompt,
  clickOperationMode,
  detectMediaProviderGate,
  ensureProgramPage,
  ensureSharePage,
  inferInvokeUiState,
  isBrowserDownloadAssetUrl,
  isTransientAssistantStatusLine,
  normalizeAssistantText,
  operationConfig,
  operationModeAppearsSelected,
  resolveRuntimeStateSource,
  scopeGeminiUrlToAuthUser,
  syncEmbeddedStorageStateIntoContext,
  shouldRetryMediaPromptSubmission,
  shouldBlockMediaProviderGate,
  shouldStayOnCanvasProxyDiscoverySurface,
  submitPrompt,
  textLooksLikeGenericWelcome,
  tryFollowShareEntryPoint,
};
`;
  const tempPath = path.join(
    import.meta.dirname,
    `.gemini-canvas-browser-pool-testable-${process.pid}-${Date.now()}.mjs`,
  );
  await fs.writeFile(tempPath, `${source}\n${exportBlock}`, "utf8");
  return await import(pathToFileURL(tempPath).href);
}

test("auth gate allows prompt-ready Gemini surfaces even when marketing copy still mentions sign in", async () => {
  const { shouldTreatGeminiPageAsAuthBlocked } = await importScript();

  assert.equal(
    shouldTreatGeminiPageAsAuthBlocked(
      "Gemini\nSign in\nNew chat\nConversation with Gemini\nMeet Gemini, your personal AI assistant",
      { hasPromptTextbox: true },
    ),
    false,
  );
});

test("auth gate still blocks true signed-out landing pages without a prompt box", async () => {
  const { shouldTreatGeminiPageAsAuthBlocked } = await importScript();

  assert.equal(
    shouldTreatGeminiPageAsAuthBlocked(
      "Gemini\nSign in\nMeet Gemini, your personal AI assistant",
      { hasPromptTextbox: false },
    ),
    true,
  );
});

test("Gemini account scope preserves the configured Google account slot", async () => {
  const { applyGeminiAccountScope, scopeGeminiUrlToAuthUser } = await importTestableScript();

  const scoped = applyGeminiAccountScope({
    authUser: "1",
    baseUrl: "https://gemini.google.com",
    canvasProgramUrl: "https://gemini.google.com/app/4abc4e7577b6149f",
    pageUrl: "https://gemini.google.com/u/0/app/4abc4e7577b6149f",
  });

  assert.equal(scoped.baseUrl, "https://gemini.google.com/u/1/");
  assert.equal(
    scoped.canvasProgramUrl,
    "https://gemini.google.com/u/1/app/4abc4e7577b6149f",
  );
  assert.equal(
    scoped.pageUrl,
    "https://gemini.google.com/u/1/app/4abc4e7577b6149f",
  );
  assert.equal(
    scopeGeminiUrlToAuthUser(
      "https://gemini.google.com/u/0/share/fe24c455a570",
      "1",
    ),
    "https://gemini.google.com/share/fe24c455a570?authuser=1",
  );
});

test("reuse inspection recreates explicitly closed contexts before newPage is attempted", async () => {
  const { inspectContextEntryForReuse } = await importScript();

  assert.deepEqual(
    inspectContextEntryForReuse({
      context: {
        pages() {
          return [];
        },
      },
      contextClosed: true,
    }),
    {
      recreate: true,
      adoptedPage: false,
    },
  );
});

test("reuse inspection recreates contexts whose owning browser is disconnected", async () => {
  const { inspectContextEntryForReuse } = await importScript();

  assert.deepEqual(
    inspectContextEntryForReuse({
      context: {
        pages() {
          return [];
        },
        browser() {
          return {
            isConnected() {
              return false;
            },
          };
        },
      },
    }),
    {
      recreate: true,
      adoptedPage: false,
    },
  );
});

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

test("browser download detection includes contribution usercontent assets", async () => {
  const { isBrowserDownloadAssetUrl } = await importTestableScript();

  assert.equal(
    isBrowserDownloadAssetUrl("https://contribution.usercontent.google.com/download?filename=video.mp4"),
    true,
  );
});

test("runtime state resolution prefers relaunching a persistent profile directory when Chromium profile markers exist", async () => {
  const { resolveRuntimeStateSource } = await importTestableScript();

  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "gemini-canvas-runtime-source-"));
  const profileDir = path.join(tempDir, "manual-profile");
  const defaultDir = path.join(profileDir, "Default");
  const storageStatePath = path.join(profileDir, "storage-state.json");
  const localStatePath = path.join(profileDir, "Local State");
  await fs.mkdir(defaultDir, { recursive: true });
  await fs.writeFile(storageStatePath, JSON.stringify({ cookies: [], origins: [] }), "utf8");
  await fs.writeFile(localStatePath, JSON.stringify({ profile: { info_cache: { Default: {} } } }), "utf8");

  try {
    const resolved = await resolveRuntimeStateSource(profileDir);

    assert.equal(resolved.mode, "profile_dir");
    assert.equal(resolved.absolutePath, profileDir);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});

test("runtime state resolution still uses storage-state.json for non-profile directories", async () => {
  const { resolveRuntimeStateSource } = await importTestableScript();

  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "gemini-canvas-runtime-source-"));
  const profileDir = path.join(tempDir, "storage-state-only");
  const storageStatePath = path.join(profileDir, "storage-state.json");
  await fs.mkdir(profileDir, { recursive: true });
  await fs.writeFile(storageStatePath, JSON.stringify({ cookies: [], origins: [] }), "utf8");

  try {
    const resolved = await resolveRuntimeStateSource(profileDir);

    assert.equal(resolved.mode, "storage_state_file");
    assert.equal(resolved.absolutePath, storageStatePath);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});

test("embedded storage-state cookies can hydrate a persistent profile context", async () => {
  const { syncEmbeddedStorageStateIntoContext } = await importTestableScript();

  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "gemini-canvas-storage-state-sync-"));
  const profileDir = path.join(tempDir, "persistent-profile");
  const storageStatePath = path.join(profileDir, "storage-state.json");
  const addedCookies = [];
  await fs.mkdir(profileDir, { recursive: true });
  await fs.writeFile(
    storageStatePath,
    JSON.stringify({
      cookies: [
        {
          name: "SID",
          value: "sid-value",
          domain: ".google.com",
          path: "/",
          expires: -1,
          httpOnly: true,
          secure: true,
          sameSite: "None",
        },
      ],
      origins: [],
    }),
    "utf8",
  );

  const context = {
    async addCookies(cookies) {
      addedCookies.push(...cookies);
    },
  };

  try {
    const count = await syncEmbeddedStorageStateIntoContext(context, profileDir);

    assert.equal(count, 1);
    assert.equal(addedCookies.length, 1);
    assert.equal(addedCookies[0].name, "SID");
    assert.equal(addedCookies[0].domain, ".google.com");
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});

test("share entry follow keeps stepping when the first click does not materialize an app surface", async () => {
  const { tryFollowShareEntryPoint } = await importTestableScript();

  const clickOrder = [];
  const state = {
    currentUrl: "https://gemini.google.com/share/fe24c455a570",
    continueVisible: false,
  };

  function makeCandidate(kind, exists, onClick) {
    return {
      first() {
        return this;
      },
      async count() {
        return typeof exists === "function" ? (exists() ? 1 : 0) : exists ? 1 : 0;
      },
      async waitFor() {},
      async click() {
        clickOrder.push(kind);
        onClick?.();
      },
    };
  }

  const candidates = new Map([
    [
      "try",
      makeCandidate("try", true, () => {
        state.continueVisible = true;
      }),
    ],
    [
      "continue",
      makeCandidate("continue", () => state.continueVisible, () => {
        state.currentUrl = "https://gemini.google.com/app/4abc4e7577b6149f";
      }),
    ],
    ["image", makeCandidate("image", false)],
    ["open", makeCandidate("open", false)],
  ]);

  const page = {
    locator(selector = "") {
      if (String(selector).includes('button[data-test-id="copy-canvas-button"]')) {
        return makeCandidate("copy_canvas", false);
      }
      return {
        first() {
          return this;
        },
        filter({ hasText }) {
          const pattern = String(hasText);
          if (pattern.includes("Try Gemini Canvas")) {
            return candidates.get("try");
          }
          if (pattern.includes("Continue")) {
            return candidates.get("continue");
          }
          if (pattern.includes("Create image")) {
            return candidates.get("image");
          }
          if (pattern.includes("Open in new window")) {
            return candidates.get("open");
          }
          return makeCandidate("missing", false);
        },
      };
    },
    waitForEvent() {
      return Promise.reject(new Error("no popup"));
    },
    async waitForTimeout() {},
    url() {
      return state.currentUrl;
    },
  };

  const result = await tryFollowShareEntryPoint(page);

  assert.deepEqual(clickOrder, ["try", "continue"]);
  assert.equal(result.kind, "same_page");
  assert.equal(result.page, page);
  assert.equal(state.currentUrl, "https://gemini.google.com/app/4abc4e7577b6149f");
});

test("share entry follow waits for delayed authenticated canvas materialization", async () => {
  const { tryFollowShareEntryPoint } = await importTestableScript();

  const clickOrder = [];
  const state = {
    currentUrl: "https://gemini.google.com/share/fe24c455a570",
    continueVisible: true,
    copyCanvasVisible: false,
    waitTicksSinceCopy: 0,
  };

  function makeCandidate(kind, exists, onClick) {
    return {
      first() {
        return this;
      },
      async count() {
        return typeof exists === "function" ? (exists() ? 1 : 0) : exists ? 1 : 0;
      },
      async waitFor() {},
      async click() {
        clickOrder.push(kind);
        onClick?.();
      },
    };
  }

  const missingCandidate = makeCandidate("missing", false);
  const promptCandidate = {
    first() {
      return this;
    },
    async count() {
      return state.currentUrl.includes("/canvas") ? 1 : 0;
    },
  };

  const candidates = new Map([
    [
      "continue",
      makeCandidate("continue", () => state.continueVisible, () => {
        state.continueVisible = false;
        state.copyCanvasVisible = true;
      }),
    ],
    [
      "copy_canvas",
      makeCandidate("copy_canvas", () => state.copyCanvasVisible, () => {
        state.copyCanvasVisible = false;
      }),
    ],
    ["try", missingCandidate],
    ["image", missingCandidate],
    ["open", missingCandidate],
  ]);

  const page = {
    locator(selector) {
      if (selector.includes('[role="textbox"]') || selector.includes('[contenteditable="true"]')) {
        return promptCandidate;
      }
      if (selector.includes('button[data-test-id="copy-canvas-button"]')) {
        return candidates.get("copy_canvas");
      }
      return {
        first() {
          return this;
        },
        filter({ hasText }) {
          const pattern = String(hasText);
          if (pattern.includes("Continue")) {
            return candidates.get("continue");
          }
          if (pattern.includes("Try Gemini Canvas")) {
            return candidates.get("try");
          }
          if (pattern.includes("Create image")) {
            return candidates.get("image");
          }
          if (pattern.includes("Open in new window")) {
            return candidates.get("open");
          }
          return missingCandidate;
        },
      };
    },
    waitForEvent() {
      return Promise.reject(new Error("no popup"));
    },
    async waitForTimeout() {
      if (!state.copyCanvasVisible && !state.currentUrl.includes("/canvas")) {
        state.waitTicksSinceCopy += 1;
        if (state.waitTicksSinceCopy >= 2) {
          state.currentUrl = "https://gemini.google.com/canvas";
        }
      }
    },
    async waitForLoadState() {},
    async evaluate() {
      if (state.currentUrl.includes("/canvas")) {
        return "Conversation with Gemini\nBrowser API Proxy Client\nCode\nPreview";
      }
      if (state.copyCanvasVisible) {
        return "Gemini\nTry Gemini Canvas";
      }
      return "Gemini\nContinue";
    },
    url() {
      return state.currentUrl;
    },
  };

  const result = await tryFollowShareEntryPoint(page);

  assert.deepEqual(clickOrder, ["continue", "copy_canvas"]);
  assert.equal(result.kind, "same_page");
  assert.equal(result.page, page);
  assert.equal(state.currentUrl, "https://gemini.google.com/canvas");
});

test("assistant text normalization discards transient Gemini reasoning status labels", async () => {
  const { isTransientAssistantStatusLine, normalizeAssistantText } =
    await importTestableScript();

  assert.equal(isTransientAssistantStatusLine("Pinpointing the Query"), true);
  assert.equal(isTransientAssistantStatusLine("Searching the web"), true);
  assert.equal(isTransientAssistantStatusLine("Identifying the Focus"), true);
  assert.equal(isTransientAssistantStatusLine("Formulating a Response"), true);
  assert.equal(normalizeAssistantText("Pinpointing the Query\nGemini said"), "");
  assert.equal(normalizeAssistantText("Searching the web\nGemini says"), "");
  assert.equal(normalizeAssistantText("Identifying the Focus\nGemini said"), "");
});

test("assistant text normalization preserves the actual Gemini answer", async () => {
  const { normalizeAssistantText } = await importTestableScript();

  assert.equal(
    normalizeAssistantText("Pinpointing the Query\nGemini said\n法国的首都是巴黎。"),
    "法国的首都是巴黎。",
  );
  assert.equal(
    normalizeAssistantText("Paris is the capital of France."),
    "Paris is the capital of France.",
  );
});

test("text response validation rejects a stale generic welcome", async () => {
  const { bodyContainsSubmittedPrompt, textLooksLikeGenericWelcome } =
    await importTestableScript();

  assert.equal(
    bodyContainsSubmittedPrompt(
      "Conversation with Gemini\nYou said\n法国的首都在哪里",
      "法国的首都在哪里",
    ),
    true,
  );
  assert.equal(
    textLooksLikeGenericWelcome(
      "法国的首都在哪里",
      "Hello! How can I help you today? Feel free to ask a question.",
    ),
    true,
  );
  assert.equal(textLooksLikeGenericWelcome("hello", "Hello! How can I help you today?"), false);
  assert.equal(textLooksLikeGenericWelcome("法国的首都在哪里", "法国的首都是巴黎。"), false);
});

test("canvas proxy discovery opens preview when the proxy client is visibly materialized", async () => {
  const { shouldStayOnCanvasProxyDiscoverySurface } = await importTestableScript();

  assert.equal(
    shouldStayOnCanvasProxyDiscoverySurface(
      true,
      { bodyText: "Browser API Proxy Client\nCode\nPreview" },
      { handlePairs: [], invokeContract: null },
    ),
    true,
  );
  assert.equal(
    shouldStayOnCanvasProxyDiscoverySurface(
      false,
      { bodyText: "Browser API Proxy Client\nCode\nPreview" },
      { handlePairs: [], invokeContract: null },
    ),
    false,
  );
  assert.equal(
    shouldStayOnCanvasProxyDiscoverySurface(
      true,
      { bodyText: "Conversation with Gemini" },
      { handlePairs: [], invokeContract: null },
    ),
    false,
  );
});

test("canvas proxy preview falls back to direct launch when Canvas blocks its local websocket", async () => {
  const { canvasProxyPreviewNeedsDirectLaunch } = await importTestableScript();
  const preview = {
    bridge: { eventCount: 1 },
    bridgeEvents: [
      {
        type: "log",
        errorMessage:
          "WebSocket initialization failed. WebSocket connection to 'wss://127.0.0.1:9998?authIndex=0' is not allowed in Canvas.",
      },
      { type: "log", errorMessage: "System initializing..." },
    ],
  };

  assert.equal(canvasProxyPreviewNeedsDirectLaunch(preview, "Something went wrong", true), true);
  assert.equal(canvasProxyPreviewNeedsDirectLaunch(preview, "Something went wrong", false), false);
  assert.equal(
    canvasProxyPreviewNeedsDirectLaunch(preview, "System Logs Output\nConnected", true),
    false,
  );
});
