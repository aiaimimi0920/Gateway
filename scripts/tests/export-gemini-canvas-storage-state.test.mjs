import assert from "node:assert/strict";
import test from "node:test";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { pathToFileURL } from "node:url";

const execFileAsync = promisify(execFile);

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "export-gemini-canvas-storage-state.mjs",
);

async function importScript() {
  return await import(pathToFileURL(scriptPath).href);
}

test("Gemini Canvas storage-state help exits before resolving browser launch", async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), "gemini-canvas-export-help-"));
  const fakeBrowserPath = path.join(tempDir, "fake-browser.exe");
  await writeFile(fakeBrowserPath, "");

  try {
    const { stdout, stderr } = await execFileAsync(
      process.execPath,
      [scriptPath, "--help"],
      {
        env: {
          ...process.env,
          GEMINI_CANVAS_CAPTURE_BROWSER_EXECUTABLE_PATH: fakeBrowserPath,
        },
        timeout: 10_000,
      },
    );

    assert.match(stdout, /Usage: node scripts\/export-gemini-canvas-storage-state\.mjs/);
    assert.match(stdout, /GEMINI_CANVAS_CAPTURE_TIMEOUT_MS/);
    assert.equal(stderr, "");
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test("Gemini Canvas auth signal accepts a logged-in Gemini surface even before the prompt box appears", async () => {
  const { isGeminiAuthenticatedSignal } = await importScript();

  assert.equal(
    isGeminiAuthenticatedSignal({
      url: "https://gemini.google.com/app",
      cookies: [
        {
          name: "SAPISID",
          domain: ".google.com",
        },
      ],
      pageSignal: {
        loginVisible: false,
        hasGeminiSurface: true,
        hasConversationInput: false,
        hasNonDefaultAvatar: false,
        marketingVisible: true,
        title: "Google Gemini",
      },
    }),
    true,
  );
});

test("Gemini Canvas auth signal rejects non-Google cookie domains", async () => {
  const { hasGoogleAuthCookies, isGeminiAuthenticatedSignal } = await importScript();

  assert.equal(
    hasGoogleAuthCookies([
      {
        name: "SAPISID",
        domain: "evilgoogle.com",
      },
    ]),
    false,
  );

  assert.equal(
    isGeminiAuthenticatedSignal({
      url: "https://gemini.google.com/app",
      cookies: [
        {
          name: "SAPISID",
          domain: "evilgoogle.com",
        },
      ],
      pageSignal: {
        loginVisible: false,
        hasGeminiSurface: true,
        hasConversationInput: true,
        hasNonDefaultAvatar: false,
        marketingVisible: false,
        title: "Google Gemini",
      },
    }),
    false,
  );
});

test("Gemini Canvas helper recognizes Google secondary verification challenges", async () => {
  const { isGoogleVerificationChallengeVisible, isGeminiAuthenticatedSignal } =
    await importScript();

  const pageSignal = {
    loginVisible: false,
    verificationChallengeVisible: isGoogleVerificationChallengeVisible({
      title: "为了您的账号安全，请验证后登录。",
      bodyText: "选择所有符合描述的图片 包含文字: “王”",
    }),
    hasGeminiSurface: true,
    hasConversationInput: true,
    hasNonDefaultAvatar: true,
    marketingVisible: false,
    title: "Google Gemini",
  };

  assert.equal(pageSignal.verificationChallengeVisible, true);
  assert.equal(
    isGeminiAuthenticatedSignal({
      url: "https://gemini.google.com/app",
      cookies: [
        {
          name: "SAPISID",
          domain: ".google.com",
        },
      ],
      pageSignal,
    }),
    false,
  );
});

test("Gemini Canvas manual-complete fallback trusts a concrete Gemini app handle even if loginVisible is noisy", async () => {
  const { canForceCompleteGeminiCapture } = await importScript();

  assert.equal(
    canForceCompleteGeminiCapture({
      url: "https://gemini.google.com/u/1/app/5344b4d4494a21d5",
      cookies: [
        {
          name: "SAPISID",
          domain: ".google.com",
        },
      ],
      pageSignal: {
        hasGeminiSurface: true,
        loginVisible: true,
        verificationChallengeVisible: false,
      },
    }),
    true,
  );
});

test("Gemini Canvas manual-complete fallback still rejects Google verification challenges", async () => {
  const { canForceCompleteGeminiCapture } = await importScript();

  assert.equal(
    canForceCompleteGeminiCapture({
      url: "https://gemini.google.com/app",
      pageSignal: {
        hasGeminiSurface: true,
        loginVisible: false,
        verificationChallengeVisible: true,
      },
    }),
    false,
  );
});

test("Gemini Canvas manual-complete fallback rejects a logged-out generic app page", async () => {
  const { canForceCompleteGeminiCapture } = await importScript();

  assert.equal(
    canForceCompleteGeminiCapture({
      url: "https://gemini.google.com/app",
      cookies: [
        {
          name: "SAPISID",
          domain: ".google.com",
        },
      ],
      pageSignal: {
        hasGeminiSurface: true,
        hasConversationInput: true,
        hasAccountMenuButton: false,
        hasNonDefaultAvatar: false,
        loginVisible: true,
        verificationChallengeVisible: false,
        marketingVisible: false,
      },
    }),
    false,
  );
});

test("Gemini Canvas candidate picker prefers the page that actually exposes Gemini auth signals", async () => {
  const { pickGeminiCandidatePage } = await importScript();

  const selected = pickGeminiCandidatePage([
    {
      url: "https://gemini.google.com/",
      pageSignal: {
        hasGeminiSurface: true,
        hasConversationInput: false,
        hasNonDefaultAvatar: false,
        hasAccountMenuButton: false,
      },
    },
    {
      url: "https://gemini.google.com/app",
      pageSignal: {
        hasGeminiSurface: true,
        hasConversationInput: false,
        hasNonDefaultAvatar: true,
        hasAccountMenuButton: true,
      },
    },
  ]);

  assert.equal(selected?.url, "https://gemini.google.com/app");
});

test("Gemini Canvas runtime capture extracts unique Google API keys from page blobs", async () => {
  const { extractGoogleApiKeysFromBlob } = await importScript();

  const keys = extractGoogleApiKeysFromBlob(`
    <script>
      window.firebaseConfig = { apiKey: "AIzaCapturedOne123456789012" };
      const duplicates = ["AIzaCapturedTwo123456789012", "AIzaCapturedOne123456789012"];
    </script>
  `);

  assert.deepEqual(keys, [
    "AIzaCapturedOne123456789012",
    "AIzaCapturedTwo123456789012",
  ]);
});
