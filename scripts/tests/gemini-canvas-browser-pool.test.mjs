import assert from "node:assert/strict";
import test from "node:test";
import { importScript, importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
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
