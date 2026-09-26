import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { app, appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

function discoveryFixture(options = {}) {
  let now = 0;
  const events = [], waits = [], stamps = [], logs = [];
  const frame = (url) => ({ url: () => url });
  const page = (name, settings = {}) => {
    const frames = settings.frames ?? [frame("https://gemini.google.com/app"), frame("blob:preview")];
    return {
      name, closed: settings.closed ?? false, closes: 0,
      url: () => `https://gemini.google.com/u/6/app/${name}`,
      isClosed() { return this.closed; },
      async close() { this.closes += 1; if (settings.closeError) throw settings.closeError; this.closed = true; },
      async waitForTimeout(ms) { waits.push([name, ms]); now += ms; },
      async evaluate() { if (settings.bodyError) throw new Error("detached body"); return settings.body ?? "Browser API Proxy Client"; },
      frames: () => frames,
    };
  };
  const original = page("original", options.original);
  const entry = { page: original };
  const sandbox = vm.createContext({ Error, String, Math, Date: { now: () => now },
    ensureSharePage: async (...args) => { events.push(["share", ...args]); if (options.shareError) throw options.shareError; },
    tryFollowShareEntryPoint: async (active) => {
      events.push(["follow", active]);
      return { page: options.follow ? await options.follow(active, page) : active };
    },
    tryOpenCanvasProxyPreview: async (active, timeout) => {
      events.push(["open", active, timeout]);
      return options.open ? await options.open(active, page) : { clicked: true };
    },
    stampCanvasProxyPreviewFrames: async (...args) => {
      stamps.push(args);
      return options.stamp ? options.stamp(stamps.length, ...args) : [{ index: 1, stamped: true }];
    },
    inferGoogleAuthUser: app.inferGoogleAuthUser,
    log: (...args) => logs.push(args),
  });
  // Execute the original root-exported functions before and after extraction;
  // the offline browser contract below separately exercises real factory wiring.
  const run = vm.runInContext(`${app.isCanvasProxyPreviewFrameUrl.toString()}; (${app.ensureCanvasProxyPreviewFrame.toString()})`, sandbox);
  return { entry, original, events, waits, stamps, logs, frame,
    run: (timeout = 5000, request = null) => run(entry, "https://gemini.google.com/u/6", "synthetic-share", timeout, request) };
}

test("preview frame selection prefers a successful probe and forwards the exact custom request", async () => {
  const frames = ["https://gemini.google.com/app", "blob:first", "blob:successful"].map((url) => ({ url: () => url }));
  const stamped = [{ index: 1, stamped: true }, { index: 8, stamped: true, probeFetchResult: { ok: true } }, { index: 2, stamped: true, probeFetchResult: { ok: true } }];
  const f = discoveryFixture({ original: { frames }, stamp: () => stamped });
  const request = { url: "https://fixture.invalid/probe", bodyText: "synthetic" };
  const result = await f.run(5000, request);
  assert.equal(result.frame, frames[2]);
  assert.equal(result.page, f.original);
  assert.equal(result.stampedFrames, stamped);
  assert.equal(result.preview.clicked, true);
  assert.deepEqual(f.stamps, [[f.original, "6", request]]);
  assert.deepEqual(f.events.map((event) => event[0]), ["share", "follow", "open"]);
  assert.deepEqual(f.waits, [["original", 2500], ["original", 1200]]);
  assert.equal(f.original.closes, 0);
});

test("preview frame selection retains the first stamped frame when all probes fail", async () => {
  const f = discoveryFixture({ stamp: () => [{ index: 99, stamped: true }, { index: 1, stamped: true, probeFetchResult: { ok: false } }] });
  const result = await f.run();
  assert.equal(result.frame, f.original.frames()[1]);
});

test("preview frame URL fallback ignores the main frame and preserves existing URL admission", async () => {
  const frames = ["blob:main", "https://unrelated.invalid/", "https://scf.usercontent.goog.example.invalid/frame"].map((url) => ({ url: () => url }));
  const f = discoveryFixture({ original: { frames }, stamp: () => [] });
  assert.equal((await f.run()).frame, frames[2]);
  assert.equal(app.isCanvasProxyPreviewFrameUrl("BLOB:fixture"), true);
  assert.equal(app.isCanvasProxyPreviewFrameUrl(null), false);
});

test("preview page adoption closes replaced pages and retains the final preview page", async () => {
  let followed, preview;
  const f = discoveryFixture({
    original: { closeError: new Error("original close failed") },
    follow: (_, page) => (followed = page("followed")),
    open: (_, page) => ({ clicked: true, page: (preview = page("preview")) }),
  });
  const result = await f.run();
  assert.equal(f.original.closes, 1);
  assert.equal(followed.closes, 1);
  assert.equal(followed.isClosed(), true);
  assert.equal(preview.closes, 0);
  assert.equal(f.entry.page, preview);
  assert.equal(result.page, preview);
  assert.equal(f.stamps[0][0], preview);
});

test("preview page adoption skips closing already closed pages and repeated page identity", async () => {
  let followed;
  const f = discoveryFixture({ original: { closed: true },
    follow: (_, page) => (followed = page("followed")), open: (active) => ({ page: active }) });
  await f.run();
  assert.equal(f.original.closes, 0);
  assert.equal(followed.closes, 0);
});

test("preview discovery retries the share entry once when body evaluation is unavailable", async () => {
  const f = discoveryFixture({ original: { bodyError: true }, follow: () => null });
  await f.run();
  assert.deepEqual(f.events.map((event) => event[0]), ["share", "follow", "share", "follow", "open"]);
  assert.deepEqual(f.waits, [["original", 2500], ["original", 2500], ["original", 1200]]);
  assert.equal(f.original.closes, 0);
});

for (const [timeout, polls] of [[1, 8], [90000, 20]]) {
  test(`preview missing frame clamps polling and preserves error metadata for timeout ${timeout}`, async () => {
    const body = "Browser API Proxy Client " + "x".repeat(1000);
    const f = discoveryFixture({ original: { body, frames: [{ url: () => "blob:main" }] }, stamp: () => [] });
    await assert.rejects(f.run(timeout), (error) => {
      assert.equal(error.status, 503);
      assert.equal(error.code, "gemini_canvas_preview_frame_missing");
      assert.equal(error.message, "Gemini Canvas preview frame was not available for no-key invocation.");
      assert.equal(error.bodyText, body.slice(0, 800));
      return true;
    });
    assert.equal(f.stamps.length, polls);
    assert.equal(f.waits.filter(([, ms]) => ms === 1000).length, polls);
    assert.equal(JSON.parse(f.logs[0][1]).bodyPreview.length, 500);
    assert.equal(f.entry.page, f.original);
    assert.equal(f.original.closes, 0);
  });
}

test("preview discovery propagates navigation failure without starting preview work", async () => {
  const failure = Object.assign(new Error("fresh sign-in required"), { status: 401 });
  const f = discoveryFixture({ shareError: failure });
  await assert.rejects(f.run(), (error) => error === failure);
  assert.deepEqual(f.events.map((event) => event[0]), ["share"]);
  assert.equal(f.stamps.length, 0);
  assert.equal(f.original.closes, 0);
});

test("preview discovery follows a real offline share popup, closes the old page and retains the selected frame", browserTest, async (t) => {
  const f = await appFixture(t, { initialUrl: "https://gemini.google.com/u/4/share/synthetic", html: (request) => {
    const url = new URL(request.url());
    if (url.hostname === "preview.scf.usercontent.goog") return "<main>frame ready</main>";
    if (url.pathname.includes("/share/")) return '<a href="/u/4/app/synthetic" target="_blank">Continue</a>';
    return '<main>Browser API Proxy Client Connected</main><button>Preview app</button><iframe src="https://preview.scf.usercontent.goog/frame"></iframe>';
  } });
  const probes = [];
  await f.context.route("https://generativelanguage.googleapis.com/**", async (route) => {
    probes.push(route.request().url());
    await route.fulfill({ status: 200, contentType: "application/json", body: '{"synthetic":"ok"}', headers: {
      "access-control-allow-origin": "https://preview.scf.usercontent.goog", "access-control-allow-credentials": "true",
      "access-control-allow-methods": "POST, OPTIONS", "access-control-allow-headers": "content-type",
    } });
  });
  const result = await app.ensureCanvasProxyPreviewFrame(f.entry, "https://gemini.google.com/u/4", "synthetic", 5000);
  assert.notEqual(result.page, f.page);
  assert.equal(f.page.isClosed(), true);
  assert.equal(f.entry.page, result.page);
  assert.equal(result.page.isClosed(), false);
  assert.deepEqual(f.context.pages(), [result.page]);
  assert.equal(result.frame.url(), "https://preview.scf.usercontent.goog/frame");
  assert.equal(result.preview.clicked, true);
  assert.equal(result.stampedFrames[0].chromeContextId, 4);
  assert.equal(result.stampedFrames[0].probeFetchResult.bodyText, '{"synthetic":"ok"}');
  assert.ok(probes.length >= 2);
});
