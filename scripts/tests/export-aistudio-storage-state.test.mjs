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
  "export-aistudio-storage-state.mjs",
);

async function importScript() {
  return await import(pathToFileURL(scriptPath).href);
}

test("AIStudio storage-state export help exits before resolving browser launch", async () => {
  const tempDir = await mkdtemp(path.join(os.tmpdir(), "aistudio-export-help-"));
  const fakeBrowserPath = path.join(tempDir, "fake-browser.exe");
  await writeFile(fakeBrowserPath, "");

  try {
    const { stdout, stderr } = await execFileAsync(
      process.execPath,
      [scriptPath, "--help"],
      {
        env: {
          ...process.env,
          AISTUDIO_EXPORT_BROWSER_EXECUTABLE_PATH: fakeBrowserPath,
        },
        timeout: 10_000,
      },
    );

    assert.match(stdout, /Usage: node scripts\/export-aistudio-storage-state\.mjs/);
    assert.match(stdout, /AISTUDIO_EXPORT_TIMEOUT_MS/);
    assert.equal(stderr, "");
  } finally {
    await rm(tempDir, { recursive: true, force: true });
  }
});

test("AIStudio storage-state object key is timestamped and slugged", async () => {
  const { buildAistudioStorageStateObjectKey } = await importScript();

  const objectKey = buildAistudioStorageStateObjectKey({
    accountLabel: "Manual Account A",
    now: new Date("2026-06-05T13:14:15.000Z"),
  });

  assert.equal(
    objectKey,
    "credential-runtime/aistudio-web/manual-account-a-20260605-131415/storage-state.json",
  );
});

test("AIStudio storage-state object key rejects path traversal", async () => {
  const { buildAistudioStorageStateObjectKey } = await importScript();

  assert.throws(
    () =>
      buildAistudioStorageStateObjectKey({
        objectKey: "../outside-runtime",
      }),
    /unsafe.*object key/i,
  );

  assert.throws(
    () =>
      buildAistudioStorageStateObjectKey({
        objectKey: "credential-runtime/aistudio-web/..\\outside",
      }),
    /unsafe.*object key/i,
  );

  assert.throws(
    () =>
      buildAistudioStorageStateObjectKey({
        objectKey: "C:/outside-runtime",
      }),
    /unsafe.*object key/i,
  );
});

test("AIStudio auth signal requires an AIStudio surface and Google auth cookies", async () => {
  const { isAistudioAuthenticatedSignal } = await importScript();

  assert.equal(
    isAistudioAuthenticatedSignal({
      url: "https://ai.studio/apps/example",
      cookies: [
        {
          name: "SAPISID",
          domain: ".google.com",
        },
      ],
      pageSignal: {
        loginVisible: false,
        hasAistudioSurface: true,
        hasPromptInput: false,
      },
    }),
    true,
  );

  assert.equal(
    isAistudioAuthenticatedSignal({
      url: "https://accounts.google.com/v3/signin/identifier",
      cookies: [
        {
          name: "SAPISID",
          domain: ".google.com",
        },
      ],
      pageSignal: {
        loginVisible: true,
        hasAistudioSurface: false,
        hasPromptInput: false,
      },
    }),
    false,
  );

  assert.equal(
    isAistudioAuthenticatedSignal({
      url: "https://ai.studio/apps/example",
      cookies: [
        {
          name: "SAPISID",
          domain: "evilgoogle.com",
        },
      ],
      pageSignal: {
        loginVisible: false,
        hasAistudioSurface: true,
        hasPromptInput: false,
      },
    }),
    false,
  );
});
