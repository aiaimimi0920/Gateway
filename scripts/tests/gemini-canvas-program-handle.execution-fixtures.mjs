import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import vm from "node:vm";
import { EventEmitter } from "node:events";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";
import { fixtureResponseCapture } from "./gemini-canvas-program-handle.network-fixtures.mjs";
import { createProgramNetworkCaptureOwner } from "../gemini-canvas-program-handle-network-capture.mjs";

const parameters = [
  "chromium", "profileDir", "executablePath", "shareUrl", "appUrl", "operation", "prompt",
  "discoveryOnly", "timeoutMs", "outDir", "collectSnapshot", "startNetworkCapture",
  "strongestHandle", "hasTransportHints", "toObjectKeyFromLocalPath", "normalizeString",
  "mergeInvokeContract", "buildCanvasProgramInvokeContract",
];

function executionBody() {
  const scripts = path.resolve(import.meta.dirname, "..");
  const source = fs.readFileSync(path.join(scripts, "probe-gemini-canvas-program-handle.mjs"), "utf8");
  const marker = "const context = await chromium.launchPersistentContext(";
  if (source.includes(marker)) {
    assert.equal(source.split(marker).length, 2);
    return "async function executeProgramHandleProbe({ " + parameters.join(", ") + " }) {\n" + source.slice(source.indexOf(marker)) + "\n}";
  }
  assert.equal(source.split("await executeProgramHandleProbe(").length, 2);
  const owner = fs.readFileSync(path.join(scripts, "gemini-canvas-program-handle-execution.mjs"), "utf8");
  const start = "export async function executeProgramHandleProbe(";
  assert.equal(owner.split(start).length, 2);
  return owner.slice(owner.indexOf(start)).replace("export ", "");
}

export async function executionFixture(t, options = {}) {
  const outDir = fs.mkdtempSync(path.join(os.tmpdir(), "gateway-program-handle-execution-"));
  t.after(() => fs.rmSync(outDir, { recursive: true, force: true }));
  const operation = options.operation ?? "text";
  const app = await importTestableProgramHandle({ outDir, operation });
  const calls = [], captures = [], writes = [], printed = [], snapshots = [];
  let now = 1000, snapshotIndex = 0, followIndex = 0;
  const failure = new Error("execution fixture failure");
  const step = (name, ...args) => {
    calls.push([name, ...args]);
    if (options.failAt === name) throw failure;
  };
  class Page extends EventEmitter {
    constructor(name) { super(); this.name = name; this.currentUrl = "about:blank"; this.closed = false; }
    async goto(url, settings) { step("goto", this.name, url, settings); this.currentUrl = url; }
    async waitForTimeout(ms) { step("wait", this.name, ms); now += ms; }
    url() { return this.currentUrl; }
    isClosed() { return this.closed; }
    async close() { step("page close", this.name); this.closed = true; }
    context() { return context; }
  }
  const initial = new Page("initial"), popup = new Page("popup");
  const context = Object.assign(new EventEmitter(), {
    pages() { step("pages"); return options.emptyContext ? [] : [initial]; },
    async newPage() { step("new page"); return initial; },
    async cookies() { return []; },
    async close() { step("context close"); },
  });
  const captureOwner = createProgramNetworkCaptureOwner({ ...app, operation, createResponseCapture: fixtureResponseCapture });
  const runtime = {
    ...app, outDir, operation, profileDir: "fixture-profile", executablePath: "fixture-browser",
    shareUrl: "https://gemini.google.com/share/abcdef123456", appUrl: "https://gemini.google.com/app",
    prompt: "fixture prompt", discoveryOnly: options.discoveryOnly ?? true, timeoutMs: options.timeoutMs ?? 3600,
    chromium: { async launchPersistentContext(profile, config) { step("launch", profile, config); return context; } },
    toObjectKeyFromLocalPath(value) { step("object key", value); return "fixture/profile"; },
    startNetworkCapture(page, existingState) {
      step("capture", page.name, existingState);
      const capture = captureOwner.startNetworkCapture(page, existingState), stop = capture.stop, adopt = capture.adoptPage;
      capture.ready = Promise.all([capture.ready, options.captureReady]).then(() => step("capture ready"));
      capture.stop = () => { step("capture stop", page.name); return stop(); };
      capture.adoptPage = async (nextPage) => { step("capture adopt", nextPage.name); await adopt(nextPage); };
      if (!existingState && options.seedState) options.seedState(capture.state);
      captures.push(capture);
      return capture;
    },
    async collectSnapshot(page, label) {
      step("snapshot " + label, page.name);
      const snapshot = {
        label, url: "https://gemini.google.com/app/abcdef123456", bodyText: "fixture body",
        buttons: [], anchors: [], mediaNodes: [], textboxes: [],
        ...options.snapshot?.(label, snapshotIndex++, page),
      };
      snapshot.handleHints ??= app.extractHandleHintsFromText(snapshot.url + "\n" + snapshot.bodyText);
      snapshots.push(snapshot);
      return snapshot;
    },
    buildCanvasProgramInvokeContract(...args) {
      step("invoke", args[3].label);
      return options.invoke?.(...args) ?? null;
    },
  };
  const globals = {
    ...app, path,
    Date: class extends Date { constructor(...args) { super(...(args.length ? args : [now])); } static now() { return now; } },
    console: { log(value) { step("stdout"); printed.push(value); } },
    writeFileSync(file, text, encoding) {
      const name = path.basename(file); step("write " + name);
      fs.writeFileSync(file, text, encoding); writes.push(name);
    },
    async waitForShareSurface(page, timeout) { step("share surface", page.name, timeout); return true; },
    async tryFollowShareEntryPoint(page) {
      step("follow", page.name);
      const next = options.follow?.[followIndex++];
      return next === "popup" ? { kind: "popup", page: popup } : { kind: "none", page };
    },
    async hasPromptTextbox(page) { step("textbox", page.name); return options.textbox ?? true; },
    async clickNewChat(page) { step("new chat", page.name); return true; },
    async clickOperationMode(page, mode) { step("mode", page.name, mode); return true; },
    async trySelectMusicStyleCard(page, timeout) { step("style", page.name, timeout); return { clicked: true }; },
    async submitPrompt(page, value) { step("submit", page.name, value); },
    async clickMediaActionButton(page, mode, action, timeout) {
      step(action, page.name, mode, timeout); return options.mediaClicked ?? true;
    },
  };
  const execute = vm.runInNewContext("(" + executionBody() + ")", globals, { timeout: 1000 });
  return {
    app, runtime, calls, captures, writes, printed, snapshots, initial, popup, failure, outDir,
    run: () => execute(runtime),
    read: (name) => JSON.parse(fs.readFileSync(path.join(outDir, name), "utf8")),
  };
}
