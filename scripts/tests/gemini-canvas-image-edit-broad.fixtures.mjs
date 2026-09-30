import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { EventEmitter } from "node:events";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import vm from "node:vm";

const scripts = path.resolve(import.meta.dirname, "..");
const read = (name) => fs.readFileSync(path.join(scripts, name), "utf8");
const rootSource = () => read("run-gemini-canvas-image-edit-broad.mjs");
const fields = ["events", "markerTransport", "markerResponseSeenAt", "markerRequestSeenAt", "secondarySignalerPollUrl", "secondarySignalerPollSeenAt", "secondarySignalerPollCompletedAt", "finalizeUploadCapture"];
const stripImports = (source) => source.replace(/^import .+?;\r?\n/gm, "").replace(/^export /gm, "");

function between(source, start, end) {
  assert.equal(source.split(start).length, 2, start);
  assert.equal(source.split(end).length, 2, end);
  return source.slice(source.indexOf(start), source.indexOf(end)).trimEnd();
}

function ownerSources() {
  const source = rootSource();
  if (source.includes("const capture = createImageEditBroadCapture(")) {
    return [read("gemini-canvas-image-edit-broad-capture.mjs"), read("gemini-canvas-image-edit-broad-page.mjs")].map(stripImports).join("\n");
  }
  const state = between(source, "const events = [];", "async function uploadReferenceImage(");
  const attach = between(source, 'cdp.on("Network.requestWillBeSent"', 'await page.goto("https://gemini.google.com/app"');
  const snapshot = between(source, "await page.evaluate(() => {", "\n\n  hasGeneratedImage").slice(0, -2);
  const blobs = between(source, "const pageBlobCaptures =", "const storageStatePath =").replace("exportedPageBlobs = pageBlobCaptures.map(", "return pageBlobCaptures.map(");
  return [
    "function createImageEditBroadCapture({ marker, outDir }) {", state,
    "function attach(cdp, page) {", attach, "}",
    "return { attach, " + fields.map((name) => "get " + name + "() { return " + name + "; }").join(", ") + " }; }",
    "async function collectImageEditBroadPageState(page) { return " + snapshot + "; }",
    "async function exportImageEditBroadPageBlobs(page, outDir) {", blobs, "}",
  ].join("\n");
}

export function temporaryRoot(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "gateway-image-edit-broad-"));
  t.after(() => {
    assert.equal(path.dirname(path.resolve(root)), path.resolve(os.tmpdir()));
    assert.ok(path.basename(root).startsWith("gateway-image-edit-broad-"));
    fs.rmSync(root, { recursive: true, force: true });
  });
  return root;
}

export function loadOwners(options = {}) {
  return vm.runInNewContext(ownerSources() + "\n({ createImageEditBroadCapture, collectImageEditBroadPageState, exportImageEditBroadPageBlobs })", {
    Buffer, createHash, writeFileSync: fs.writeFileSync, path, createProgramResponseCapture: fixtureResponseCapture, ...options,
  }, { timeout: 1000 });
}

function fixtureResponseCapture(page, { onResponse }) {
  page.on("response", onResponse);
  return { ready: Promise.resolve(), stop() { page.off("response", onResponse); } };
}

export async function dispatch(emitter, event, value) {
  for (const listener of emitter.listeners(event)) await listener(value);
}

export function requestFixture(options = {}) {
  const { url = "https://gemini.google.com/StreamGenerate", method = "POST", type = "fetch", data = "", headers = {} } = options;
  return { url: () => url, method: () => method, resourceType: () => type, postData: () => data,
    headers: () => headers, allHeaders: async () => headers, postDataBuffer: async () => null, ...options.overrides };
}

export function responseFixture(request = requestFixture(), options = {}) {
  const headers = { "content-type": "application/json", "content-length": "8" };
  return { request: () => request, url: request.url, status: () => 200, headers: () => headers,
    allHeaders: async () => headers, text: async () => "response", ...options };
}

export function captureFixture(t, options = {}) {
  const outDir = temporaryRoot(t), page = new EventEmitter(), cdp = new EventEmitter();
  const calls = [], failure = new Error("fixture CDP failure");
  let now = 1000;
  cdp.send = async (command, args) => { calls.push([command, args]); if (options.send) return options.send(command, args); throw failure; };
  const DateClock = class extends Date { constructor(...args) { super(...(args.length ? args : [now])); } static now() { return now; } };
  const app = loadOwners({ Date: DateClock });
  const capture = app.createImageEditBroadCapture({ marker: "fixture-marker", outDir });
  capture.attach(cdp, page);
  return { app, capture, page, cdp, outDir, calls, failure, advance: (ms) => { now += ms; },
    emit: (event, value) => dispatch(page, event, value), emitCdp: (event, value) => dispatch(cdp, event, value) };
}

export function browserEvaluate(callback, globals) {
  return vm.runInNewContext("(" + callback.toString() + ")()", globals, { timeout: 1000 });
}

export function executionFixture(t, options = {}) {
  const storageRoot = temporaryRoot(t), calls = [], writes = [], printed = [], errors = [];
  const page = new EventEmitter(), cdp = new EventEmitter();
  const failure = new Error("broad execution fixture failure"), exit = new Error("fixture process exit");
  let now = 1000, waits = 0;
  const env = { AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: storageRoot, GEMINI_CANVAS_PROFILE_OVERRIDE: "fixture-profile", GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH: "fixture-browser", GEMINI_CANVAS_IMAGE_EDIT_MARKER: "fixture-marker", ...options.env };
  const step = (name, ...args) => { calls.push([name, ...args]); if (options.failAt === name) throw failure; };
  const locator = (kind) => ({
    first() { return this; },
    async click(settings) {
      step("click " + kind, settings);
      if (kind === "send" && !options.noMarker) {
        const request = requestFixture({ data: env.GEMINI_CANVAS_IMAGE_EDIT_MARKER });
        await dispatch(page, "request", request);
        await dispatch(page, "response", responseFixture(request));
        if (options.secondaryPoll) await dispatch(page, "request", requestFixture({ url: "https://signaler-pa.clients6.google.com/punctual/multi-watch/channel?AID=7" }));
      }
    },
    async isVisible() { return Boolean(options.styleVisible); },
  });
  Object.assign(page, {
    currentUrl: "about:blank",
    async goto(url, settings) { step("goto", url, settings); page.currentUrl = url; },
    url: () => page.currentUrl,
    async waitForTimeout(ms) { step("wait", ms); now += ms; if (++waits > 2000) throw new Error("fixture exceeded bounded polling"); },
    getByRole(role, settings) { const name = String(settings.name); return locator(name.includes("发送") ? "send" : role + " " + name); },
    getByText: (pattern) => locator("text " + pattern),
    locator,
    async waitForEvent(event, settings) { step(event, settings); return { async setFiles(file) { step("set files", file); } }; },
    keyboard: { async press(key) { step("key", key); }, async type(value, settings) { step("type", value, settings); } },
    async evaluate(callback) {
      if (callback.toString().includes("const toBase64")) { step("blob evaluate"); return options.blobs ?? []; }
      step("snapshot");
      return { imageNodes: [{ src: "blob:generated", alt: "AI 生成", width: 512, height: 512 }], anchorNodes: [], cssBgNodes: [], canvasNodes: [], buttonNodes: [], pageState: { url: page.currentUrl, title: "fixture", bodyText: "fixture" }, ...options.snapshot };
    },
    async screenshot(settings) { step("screenshot", settings); fs.writeFileSync(settings.path, "fixture screenshot"); },
    async content() { step("content"); return "<html>fixture</html>"; },
  });
  cdp.send = async (command, settings) => { step("cdp " + command, settings); };
  const context = {
    pages() { step("pages"); return options.emptyContext ? [] : [page]; },
    async newPage() { step("new page"); return page; },
    async newCDPSession(value) { step("cdp session", value === page); return cdp; },
    async storageState(settings) { step("storage state", settings); fs.writeFileSync(settings.path, JSON.stringify({ cookies: [], origins: [] })); },
    async close() { step("close"); },
  };
  const globals = {
    Buffer, createHash, path, createProgramResponseCapture: fixtureResponseCapture,
    Date: class extends Date { constructor(...args) { super(...(args.length ? args : [now])); } static now() { return now; } },
    existsSync(file) {
      if (file === "fixture-browser") return !options.noBrowser;
      if (/\\(?:msedge|chrome)\.exe$/i.test(file)) return false;
      return file === "fixture-profile" ? !options.noProfile : fs.existsSync(file);
    },
    readdirSync: (file, settings) => options.noProfile ? [] : fs.readdirSync(file, settings),
    mkdirSync: fs.mkdirSync, copyFileSync: fs.copyFileSync,
    writeFileSync(file, ...args) { step("write " + path.basename(file)); writes.push(file); fs.writeFileSync(file, ...args); },
    resolveGeminiCanvasManualLiveVendorProfileDir: () => path.join(storageRoot, "absent-vendor"),
    chromium: { async launchPersistentContext(profile, settings) { step("launch", profile, settings); return context; } },
    console: { log: (value) => { step("stdout"); printed.push(JSON.parse(value)); }, error: (value) => errors.push(JSON.parse(value)) },
    process: { env, cwd: () => storageRoot, platform: options.platform ?? "win32", exit(code) { step("exit", code); throw exit; } },
  };
  const source = stripImports(rootSource());
  const owners = source.includes("const capture = createImageEditBroadCapture(") ? ownerSources() + "\n" : "";
  const execute = vm.runInNewContext("(async () => {\n" + owners + source + "\n})", globals, { timeout: 1000 });
  return { storageRoot, page, cdp, calls, writes, printed, errors, failure, exit, run: execute,
    artifact: () => JSON.parse(fs.readFileSync(writes.find((file) => file.endsWith("image-edit-broad.json")), "utf8")) };
}
