import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import { refreshFixture } from "./gemini-canvas-browser-pool.state-refresh-fixtures.mjs";

function configure(t, root, values) {
  for (const [suffix, value] of Object.entries({ DRIVER: "local", DIR: path.join(root, "checkpoints"),
    KEY: undefined, ...values })) {
    const name = `GEMINI_CANVAS_BROWSER_AUTH_CHECKPOINT_${suffix}`;
    const previous = process.env[name];
    if (value === undefined) delete process.env[name]; else process.env[name] = value;
    t.after(() => { if (previous === undefined) delete process.env[name]; else process.env[name] = previous; });
  }
}

test("opt-in checkpoint with a missing key fails before browser creation", async t => {
  const f = refreshFixture(t);
  f.write("seed");
  configure(t, f.root, {});
  await assert.rejects(f.owner.ensureContext(f.args()), { code: "gemini_canvas_auth_checkpoint_config_invalid" });
  assert.equal(f.launch.mock.callCount(), 0);
  assert.equal(f.owner.initializingContexts.size, 0);
});

test("checkpoint config rejects relative directories without launching", async t => {
  const f = refreshFixture(t);
  f.write("seed");
  configure(t, f.root, { KEY: Buffer.alloc(32, 7).toString("base64"), DIR: "relative-checkpoint" });
  await assert.rejects(f.owner.ensureContext(f.args()), { code: "gemini_canvas_auth_checkpoint_config_invalid" });
  assert.equal(f.launch.mock.callCount(), 0);
});

test("disabled checkpoint retains the existing seed-only admission", async t => {
  const f = refreshFixture(t);
  f.write("seed");
  configure(t, f.root, { DRIVER: "disabled", KEY: "not-a-key" });
  const entry = await f.owner.ensureContext(f.args());
  assert.equal(entry.context, f.sessions[0].context);
  assert.equal(f.sessions[0].loaded.cookies[0].value, "seed");
});
