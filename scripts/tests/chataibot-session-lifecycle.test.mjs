import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const worker = fileURLToPath(new URL("../chataibot-session-worker.mjs", import.meta.url));
const loader = new URL("./fixtures/chataibot-browser-loader.mjs", import.meta.url).href;
const registration = `import { register } from 'node:module'; register(${JSON.stringify(loader)});`;

for (const failure of ["none", "probe", "launch", "close", "copy"]) {
  test(`worker preserves its response and reclaims the clone after ${failure}`, async () => {
    const root = await mkdtemp(path.join(os.tmpdir(), "chataibot-lifecycle-test-"));
    try {
      const profile = path.join(root, "source");
      const temp = path.join(root, "temp");
      const eventsPath = path.join(root, "events.jsonl");
      await mkdir(path.join(profile, "Default"), { recursive: true });
      await mkdir(temp);
      await writeFile(path.join(profile, "Local State"), "fixture-local-state");
      await writeFile(path.join(profile, "Default", "Preferences"), "fixture-preferences");
      const env = { ...process.env };
      for (const key of Object.keys(env)) {
        if (/^(CHATAIBOT_|NEURO_PROVIDER_CREDENTIAL_ROOT_DIR)/i.test(key)) delete env[key];
      }
      Object.assign(env, {
        TMP: temp, TEMP: temp, TMPDIR: temp,
        CHATAIBOT_TEST_EVENTS: eventsPath, CHATAIBOT_TEST_FAILURE: failure,
      });
      const child = spawnSync(process.execPath, [
        "--import", `data:text/javascript,${encodeURIComponent(registration)}`, worker,
      ], {
        env, encoding: "utf8", timeout: 15000,
        input: JSON.stringify({
          browserExecutablePath: process.execPath, userDataDir: profile,
          profileDirectory: "Default", writeCredentialFile: false,
        }),
      });
      assert.ifError(child.error);
      const expectedSuccess = failure === "none" || failure === "close";
      assert.equal(child.status, expectedSuccess ? 0 : 1, child.stderr);
      const lines = child.stdout.trim().split("\n");
      assert.equal(lines.length, 1);
      const result = JSON.parse(lines[0]);
      assert.equal(result.ok, expectedSuccess);
      if (expectedSuccess) {
        assert.equal(result.authToken, "fixture-token");
        assert.equal(result.cookieHeader, "token=fixture-token");
      } else {
        assert.equal(result.error.status, 500);
        assert.equal(result.error.code, failure === "copy" ? "EIO" : "chataibot_session_worker_failed");
      }
      const events = existsSync(eventsPath)
        ? (await readFile(eventsPath, "utf8")).trim().split("\n").map(JSON.parse) : [];
      assert.deepEqual(events.map((event) => event.event),
        failure === "copy" ? [] : failure === "launch" ? ["launch"] : ["launch", "close"]);
      if (events.length) assert.equal(events[0].preferences, "fixture-preferences");
      assert.deepEqual(await readdir(temp), [], "worker leaked its profile clone");
      assert.equal(await readFile(path.join(profile, "Default", "Preferences"), "utf8"), "fixture-preferences");
      assert.equal(await readFile(path.join(profile, "Local State"), "utf8"), "fixture-local-state");
    } finally {
      // root comes only from mkdtemp, never from the worker's response or event log.
      await rm(root, { recursive: true, force: true });
    }
  });
}
