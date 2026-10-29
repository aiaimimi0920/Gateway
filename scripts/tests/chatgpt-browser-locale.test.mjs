import assert from "node:assert/strict";
import test from "node:test";
import { resolveBrowserLocale } from "../chatgpt-web-session/configuration.mjs";

test("browser locale keeps explicit language ahead of HTTP preferences", () => {
  const input = { languageCode: "en-us", language: "zh-CN,zh;q=0.9" };
  assert.equal(resolveBrowserLocale(input), "en-US");
  assert.equal(input.language, "zh-CN,zh;q=0.9");
  assert.equal(resolveBrowserLocale({ oaiLanguage: "ja-JP" }), "ja-JP");
});

test("browser locale accepts legacy HTTP lists but skips wildcards and exclusions", () => {
  assert.equal(resolveBrowserLocale({ language: "zh-CN,zh;q=0.9,en;q=0.8" }), "zh-CN");
  assert.equal(resolveBrowserLocale({ acceptLanguage: "*;q=0.8,en-US;q=0,fr-FR;q=0.9" }), "fr-FR");
  assert.equal(resolveBrowserLocale({ languageCode: "invalid_tag", language: "de-DE" }), "de-DE");
});

test("missing, invalid and oversized locales safely use the default", () => {
  for (const value of [undefined, null, {}, { language: "*" }, { language: "x".repeat(4097) }, { language: 42 }]) {
    assert.equal(resolveBrowserLocale(value), "zh-CN");
  }
});
