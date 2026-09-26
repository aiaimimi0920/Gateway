import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import vm from "node:vm";

export async function importTestableExporter() {
  const root = path.resolve(import.meta.dirname, "..");
  const source = await fs.readFile(path.join(root, "export-gemini-canvas-storage-state.mjs"), "utf8");
  const temporary = path.join(root, `.gemini-canvas-export-testable-${process.pid}-${Date.now()}.mjs`);
  await fs.writeFile(temporary, source + "\nexport { collectGeminiRuntimeMaterial, inspectGeminiPages, isTransientNavigationError };\n", "utf8");
  try {
    return await import(pathToFileURL(temporary).href);
  } finally {
    await fs.rm(temporary, { force: true });
  }
}

export const fixtureApiKey = (character) => "AIza" + character.repeat(26);

export function capturePage(options = {}) {
  const url = options.url ?? "https://gemini.google.com/app/fixture";
  const calls = [], timeline = options.timeline ?? [];
  let evaluations = 0;
  const record = (stage) => { calls.push(stage); timeline.push([options.label ?? "fixture", stage]); };
  const selectors = new Map([
    ["textarea, [role='textbox'], rich-textarea", options.input ?? true],
    ["[aria-label*='Google Account'], [aria-label*='Google 帐号'], [aria-label*='Google 账号'], [aria-label*='Account']", options.account ?? false],
    ["a[href*='accounts.google.com'], a[href*='signin'], button[aria-label*='登录'], button[aria-label*='Sign in']", options.login ?? false],
  ]);
  const document = {
    body: { innerText: options.bodyText ?? "Gemini fixture" },
    title: options.title ?? "Google Gemini",
    images: options.images ?? [],
    querySelector: (selector) => selectors.get(selector) ? {} : null,
  };
  return {
    calls, timeline,
    url() { record("url"); if (options.urlError) throw options.urlError; return url; },
    async content() { record("content"); if (options.contentError) throw options.contentError; return options.html ?? ""; },
    async evaluate(callback) {
      evaluations += 1;
      record("evaluate-" + evaluations);
      if (evaluations === options.failEvaluation) throw options.evaluateError;
      return vm.runInNewContext(`(${callback.toString()})()`, {
        document,
        location: { href: url, hostname: new URL(url).hostname },
        ...options.runtime,
      }, { timeout: 1000 });
    },
  };
}
