import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const {
  applyGeminiAccountScope,
  normalizeObject,
  normalizeString,
  scopeGeminiUrlToAuthUser,
} = await importTestableScript();

test("input normalization keeps object identity and rejects non-string coercion", () => {
  const object = { value: "kept" };
  assert.equal(normalizeObject(object), object);
  assert.equal(normalizeString("  account  "), "account");
  for (const value of [null, undefined, 1, [], {}, " \n "]) {
    assert.equal(normalizeString(value), null);
  }
  for (const value of [null, undefined, 1, [], "text"]) {
    assert.deepEqual(normalizeObject(value), {});
  }
});

test("invalid account slots preserve the original argument object", () => {
  for (const authUser of [undefined, null, 1, "", "-1", "1.0", "1a"]) {
    const args = { authUser, baseUrl: "  https://gemini.google.com/app  " };
    assert.equal(applyGeminiAccountScope(args), args);
    assert.equal(scopeGeminiUrlToAuthUser(args.baseUrl, authUser), "https://gemini.google.com/app");
  }
  assert.deepEqual(applyGeminiAccountScope(null), {});
});

test("valid account slots scope all URL fields without mutating caller state", () => {
  const nested = { preserve: true };
  const args = Object.freeze({
    authUser: " 02 ",
    canvasProgramUrl: "/app/program?x=1#part",
    programUrl: "https://gemini.google.com/u/7/app/other",
    pageUrl: "https://www.gemini.google.com/app/page",
    nested,
  });
  const scoped = applyGeminiAccountScope(args);
  assert.notEqual(scoped, args);
  assert.equal(scoped.authUser, " 02 ");
  assert.equal(scoped.nested, nested);
  assert.equal(scoped.baseUrl, "https://gemini.google.com/u/02/");
  assert.equal(scoped.canvasProgramUrl, "https://gemini.google.com/u/02/app/program?x=1#part");
  assert.equal(scoped.programUrl, "https://gemini.google.com/u/02/app/other");
  assert.equal(scoped.pageUrl, "https://www.gemini.google.com/u/02/app/page");
  assert.equal(args.canvasProgramUrl, "/app/program?x=1#part");
});

test("share scope replaces authuser while retaining other query fields and fragment", () => {
  assert.equal(
    scopeGeminiUrlToAuthUser("https://gemini.google.com/u/7/share/abc?x=1&authuser=7#part", "2"),
    "https://gemini.google.com/share/abc?x=1&authuser=2#part",
  );
  assert.equal(scopeGeminiUrlToAuthUser("/share/abc", "2"), "https://gemini.google.com/share/abc?authuser=2");
  assert.equal(scopeGeminiUrlToAuthUser("/share", "2"), "https://gemini.google.com/u/2/share");
});

test("account scope leaves foreign hosts and malformed URLs unchanged", () => {
  for (const url of ["https://example.com/app", "https://gemini.google.com.example.com/app", "http://[invalid"]) {
    assert.equal(scopeGeminiUrlToAuthUser(`  ${url}  `, "2"), url);
  }
  assert.equal(scopeGeminiUrlToAuthUser("  ", "2"), null);
  assert.equal(scopeGeminiUrlToAuthUser({}, "2"), null);
  assert.equal(scopeGeminiUrlToAuthUser("/u/7/app", "2"), "https://gemini.google.com/u/2/app");
});

test("account arguments retain their existing array and explicit empty URL semantics", () => {
  const args = Object.assign(["item"], { authUser: "1", baseUrl: "", programUrl: " " });
  const scoped = applyGeminiAccountScope(args);
  assert.equal(Array.isArray(scoped), false);
  assert.equal(scoped[0], "item");
  assert.equal(scoped.baseUrl, null);
  assert.equal(scoped.programUrl, null);
  assert.equal(scoped.canvasProgramUrl, null);
  assert.equal(scoped.pageUrl, null);
  assert.equal(args.baseUrl, "");
});
