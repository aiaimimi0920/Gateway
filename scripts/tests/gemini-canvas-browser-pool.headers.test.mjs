import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const sanitizers = [app.sanitizeCanvasProxyHeaders, app.sanitizeBrowserFetchHeaders];

test("header filtering treats absent scalar and array input as an empty header map", () => {
  for (const sanitize of sanitizers) {
    for (const value of [null, undefined, false, 42, "Header: value", ["header", "value"]]) assert.deepEqual(sanitize(value), {});
  }
});

test("proxy headers remove captured transport metadata but retain application and cookie headers", () => {
  const input = Object.freeze({ Host: "fixture.invalid", CONNECTION: "keep-alive", "Content-Length": "12", Origin: "https://fixture.invalid", Referer: "https://fixture.invalid/page", "User-Agent": "fixture", "Sec-Fetch-Mode": "cors", "SEC-FETCH-SITE": "same-origin", "sec-fetch-dest": "empty", Cookie: "fixture=synthetic", Authorization: "synthetic-authorization", "Content-Type": "application/json", "Sec-CH-UA": "synthetic-agent" });
  assert.deepEqual(app.sanitizeCanvasProxyHeaders(input), { Cookie: "fixture=synthetic", Authorization: "synthetic-authorization", "Content-Type": "application/json", "Sec-CH-UA": "synthetic-agent" });
});

test("browser fetch headers also remove forbidden cookies negotiation and security prefixes", () => {
  const input = Object.freeze({ Cookie: "fixture=synthetic", Cookie2: "synthetic", "Accept-Encoding": "gzip", "Access-Control-Request-Method": "POST", "Transfer-Encoding": "chunked", Upgrade: "websocket", "Permissions-Policy": "synthetic", "SeC-CH-UA": "agent", "pRoXy-Authorization": "synthetic-proxy", Authorization: "synthetic-authorization", Accept: "application/json", "X-Request-ID": "synthetic-id" });
  assert.deepEqual(app.sanitizeBrowserFetchHeaders(input), { Authorization: "synthetic-authorization", Accept: "application/json", "X-Request-ID": "synthetic-id" });
});

test("header filtering returns a new map and preserves retained spelling values and duplicates", () => {
  const nested = Object.freeze({ synthetic: true });
  const input = Object.freeze({ "X-Request-ID": "upper", "x-request-id": "lower", "X-Empty": "", "X-Number": 0, "X-Null": null, "X-Object": nested });
  for (const sanitize of sanitizers) {
    const result = sanitize(input);
    assert.deepEqual(result, input);
    assert.notEqual(result, input);
    assert.equal(result["X-Object"], nested);
  }
});

test("header filtering excludes inherited keys and keeps own proto-shaped fields as data", () => {
  const input = Object.create({ "X-Inherited": "must-not-forward" });
  Object.defineProperty(input, "__proto__", { value: "synthetic-data", enumerable: true });
  input["X-Own"] = "retained";
  Object.freeze(input);
  for (const sanitize of sanitizers) {
    const result = sanitize(input);
    assert.deepEqual(Object.keys(result), ["__proto__", "X-Own"]);
    assert.equal(Object.getPrototypeOf(result), Object.prototype);
    assert.equal(Object.getOwnPropertyDescriptor(result, "__proto__").value, "synthetic-data");
  }
});
