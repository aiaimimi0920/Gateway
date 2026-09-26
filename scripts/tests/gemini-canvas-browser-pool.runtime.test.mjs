import assert from "node:assert/strict";
import fs from "node:fs/promises";
import test from "node:test";
import os from "node:os";
import path from "node:path";
import { importScript, importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
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
