import assert from "node:assert/strict";
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { createLoginInteraction } from "../chatgpt-web-session/browser-login-interaction.mjs";

async function fixture(t) {
  const root = await realpath(await mkdtemp(path.join(tmpdir(), "gateway-interaction-")));
  t.after(() => rm(root, { recursive: true, force: true }));
  return { root, interaction: await createLoginInteraction(root) };
}

test("interaction publishes fixed metadata and waits for the supervisor's empty consent marker", async (t) => {
  const { root, interaction } = await fixture(t);
  await assert.rejects(interaction.setPhase("private response"), /invalid_input/);
  await interaction.setPhase("login_window");
  await interaction.setPhase("session_read");
  await interaction.setPhase("verification_required");
  assert.equal(await readFile(path.join(root, "login-step-3"), "utf8"), "");
  await assert.rejects(interaction.setPhase("verification_required"), /invalid_input/);
  const abort = new AbortController();
  const waiting = interaction.waitForVerification(Date.now() + 1000, abort.signal);
  await writeFile(path.join(root, "login-verify"), "", { flag: "wx" });
  await waiting;
});

test("interaction deadline and cancellation do not authorize a browser launch", async (t) => {
  const { interaction } = await fixture(t);
  const abort = new AbortController();
  await assert.rejects(interaction.waitForVerification(Date.now() - 1, abort.signal), /verification_wait_timeout/);
  const waiting = interaction.waitForVerification(Date.now() + 1000, abort.signal);
  abort.abort();
  await assert.rejects(waiting, /verification_cancelled/);
});

test("interaction rejects command content and non-files rather than consuming arbitrary input", async (t) => {
  const { root, interaction } = await fixture(t);
  const command = path.join(root, "login-verify");
  await writeFile(command, "not a command");
  await assert.rejects(interaction.waitForVerification(Date.now() + 1000, new AbortController().signal), /invalid_input/);
  await rm(command);
  await mkdir(command);
  await assert.rejects(interaction.waitForVerification(Date.now() + 1000, new AbortController().signal), /invalid_input/);
});
