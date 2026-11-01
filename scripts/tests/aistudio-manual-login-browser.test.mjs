import assert from "node:assert/strict";
import { test } from "node:test";
import path from "node:path";
import { manualLoginArguments, cookieImportArguments, ownedProfilePath } from "../aistudio-manual-login-browser.mjs";

const root = path.resolve("fixture-private-root");
const profile = path.join(root, "aistudio-login-fixture");
const url = "https://aistudio.google.com/prompts/new_chat";

test("manual Google login has no automation driver, debug port or weakened browser security", () => {
  const args = manualLoginArguments(profile, url);
  assert.deepEqual(args, [`--user-data-dir=${profile}`, "--no-first-run", "--no-default-browser-check", url]);
  assert.doesNotMatch(args.join(" "), /automation|debugging|disable-blink|ignore-certificate|disable-web-security/i);
});

test("cookie import attaches only after manual login, to a nonzero loopback port", () => {
  const args = cookieImportArguments(profile, url, 49152);
  assert.ok(args.includes("--remote-debugging-address=127.0.0.1"));
  assert.ok(args.includes("--remote-debugging-port=49152"));
  assert.ok(args.includes(`--user-data-dir=${profile}`));
  assert.doesNotMatch(args.join(" "), /remote-allow-origins|enable-automation|disable-blink|ignore-certificate/i);
  for (const port of [0, -1, 65536, "9222"]) assert.throws(() => cookieImportArguments(profile, url, port));
});

test("FlowMusic can opt out of background mode without changing other login defaults", () => {
  const target = "https://www.flowmusic.app/";
  const args = manualLoginArguments(profile, target, true);
  assert.ok(args.includes("--disable-background-mode"));
  assert.equal(args.at(-1), target);
  assert.doesNotMatch(args.join(" "), /automation|debugging|disable-blink|ignore-certificate|disable-web-security/i);
  assert.ok(cookieImportArguments(profile, target, 49152, true).includes("--disable-background-mode"));
  assert.ok(!manualLoginArguments(profile, url).includes("--disable-background-mode"));
});

test("cleanup accepts only the exclusively created immediate temporary profile", () => {
  assert.equal(ownedProfilePath(root, profile), profile);
  for (const candidate of [root, path.dirname(root), path.resolve(root, "..", "aistudio-login-other"),
    path.join(root, "ordinary-profile"), path.join(profile, "nested")]) {
    assert.throws(() => ownedProfilePath(root, candidate));
  }
  assert.throws(() => ownedProfilePath("relative", profile));
});
