import assert from "node:assert/strict";
import test from "node:test";
import {
  buildLegacyValidationRequest,
  isProcessSessionValidationRequest,
  legacyResponseContainsParis,
} from "../gemini-web-capture-compat.mjs";

const prompt = "法国的首都在哪里";
const endpoint = "https://geminiweb-pa.clients6.google.com/v1/processSession";
const form = new URLSearchParams({
  count: "2", req0___data__: Buffer.from("session metadata").toString("base64"),
  req1___data__: Buffer.concat([Buffer.from([18, 27]), Buffer.from(prompt)]).toString("base64"),
}).toString();

test("recognizes the exact validation prompt in bounded WebChannel data only", () => {
  assert.equal(isProcessSessionValidationRequest(endpoint, form, prompt), true);
  for (const url of [endpoint.replace("clients6", "clients7"), `${endpoint}/other`,
    endpoint.replace("https:", "http:"), "https://signaler-pa.clients6.google.com/v1/processSession"]) {
    assert.equal(isProcessSessionValidationRequest(url, form, prompt), false);
  }
  assert.equal(isProcessSessionValidationRequest(endpoint, "req0___data__=!!!!", prompt), false);
  assert.equal(isProcessSessionValidationRequest(endpoint, form, "另一个问题"), false);
  assert.equal(isProcessSessionValidationRequest(endpoint, "x".repeat(512 * 1024 + 1), prompt), false);
});

test("builds a fixed legacy request without forwarding unrelated URL or header material", () => {
  const source = {
    url: "https://gemini.google.com/_/BardChatUi/data/batchexecute?bl=build&f.sid=sid&authuser=0&key=private",
    postData: new URLSearchParams({at: "access", "f.req": "rpc-data"}).toString(),
    headers: {referer: "https://gemini.google.com/app", "x-goog-authuser": "0", authorization: "unrelated"},
  };
  const request = buildLegacyValidationRequest(source, prompt);
  const url = new URL(request.url);
  assert.equal(url.hostname, "gemini.google.com");
  assert.ok(url.pathname.endsWith("/StreamGenerate"));
  assert.equal(url.searchParams.get("authuser"), "0");
  assert.equal(url.searchParams.has("key"), false);
  assert.equal(request.headers.authorization, undefined);
  assert.equal(request.form.at, "access");
  const inner = JSON.parse(JSON.parse(request.form["f.req"])[1]);
  assert.equal(inner.length, 69);
  assert.equal(inner[0][0], prompt);
  assert.equal(inner[68], 2);
  assert.throws(() => buildLegacyValidationRequest({...source, url: source.url.replace("gemini.google.com", "evil.invalid")}, prompt));
  assert.throws(() => buildLegacyValidationRequest({...source, postData: "f.req=rpc"}, prompt));
  const scoped = buildLegacyValidationRequest({...source,
    url: "https://gemini.google.com/u/0/_/BardChatUi/data/batchexecute?bl=build",
  }, prompt);
  assert.equal(new URL(scoped.url).searchParams.get("authuser"), "0");
});

test("accepts Paris only from a parsed legacy assistant candidate, never an error or HTML", () => {
  const inner = [null, null, null, null, [["candidate", ["法国的首都是巴黎。"]]]];
  const body = ")]}'\n\n123\n" + JSON.stringify([["wrb.fr", null, JSON.stringify(inner)]]);
  assert.equal(legacyResponseContainsParis(200, body), true);
  for (const [status, value] of [[401, body], [200, "<html>巴黎</html>"],
    [200, JSON.stringify({error: "巴黎"})], [200, body.replace("巴黎", "伦敦")],
    [200, body + "x".repeat(1024 * 1024)]]) {
    assert.equal(legacyResponseContainsParis(status, value), false);
  }
});
