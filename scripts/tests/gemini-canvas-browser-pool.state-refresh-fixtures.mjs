import fs from "node:fs";
import path from "node:path";
import { EventEmitter } from "node:events";
import { chromium } from "playwright-core";
import { createContextOwner } from "../gemini-canvas-browser-pool-context.mjs";
import { storageFixture } from "./gemini-canvas-browser-pool.storage-fixtures.mjs";

export const state = (value) => ({ cookies: [{ name: "fixture", value, domain: "example.invalid",
  path: "/", expires: -1, httpOnly: true, secure: true, sameSite: "Lax" }], origins: [] });

export function refreshFixture(t) {
  const root = storageFixture(t), sessions = [], logs = [], hooks = {};
  const owner = createContextOwner({ log: (...args) => logs.push(args), isFixtureCanvasBaseUrl: () => false,
    DEFAULT_TIMEOUT_MS: 1000, DEFAULT_LOCALE: "en-US", DEFAULT_IDLE_TIMEOUT_MS: 60000,
    cloneProfileDirectory: () => { throw new Error("Unexpected clone"); },
    removeManagedLaunchProfileClone: async () => {}, cleanupClonedLaunchProfile: async () => {} });
  const launch = t.mock.method(chromium, "launch", async () => {
    const browser = new EventEmitter(), context = new EventEmitter();
    const page = { isClosed: () => false }, session = { browser, context, closes: [] };
    browser.isConnected = () => true;
    browser.newContext = async (options) => {
      await hooks.beforeContext?.();
      session.input = options.storageState;
      session.loaded = typeof options.storageState === "string"
        ? JSON.parse(fs.readFileSync(options.storageState, "utf8")) : structuredClone(options.storageState);
      return context;
    };
    browser.close = async () => { session.closes.push("browser"); browser.emit("disconnected"); };
    context.pages = () => [page];
    context.browser = () => browser;
    context.close = async () => { session.closes.push("context"); context.emit("close"); };
    sessions.push(session);
    return browser;
  });
  t.after(async () => { for (const key of owner.contexts.keys()) await owner.closeContext(key); });
  const args = (key = "state.json") => ({ runtimeStateObjectKey: key, browserExecutablePath: process.execPath });
  const write = (value, key = "state.json") => {
    const file = path.join(root, key);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, JSON.stringify(state(value)), "utf8");
    return file;
  };
  return { root, sessions, launch, logs, hooks, owner, args, write };
}
