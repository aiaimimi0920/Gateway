import assert from "node:assert/strict";
import test from "node:test";
import { cookieMatchesOrigin, runtimeStateOrigin, scopeRuntimeState } from "../udio-browser/state-scope.mjs";

test("provider cookie scope distinguishes domain cookies, host cookies and lookalikes", () => {
  const target = runtimeStateOrigin("https://www.udio.com/create");
  for (const domain of [".udio.com", ".www.udio.com", "www.udio.com", ".UDIO.COM"]) {
    assert.equal(cookieMatchesOrigin({ domain }, target), true, domain);
  }
  for (const domain of ["udio.com", "eviludio.com", ".eviludio.com", "udio.com.attacker.test", "", "."]) {
    assert.equal(cookieMatchesOrigin({ domain }, target), false, domain);
  }
  assert.equal(cookieMatchesOrigin({}, target), false);
});

test("provider state scope enforces exact origin and ignores unknown top-level exports", () => {
  const origin = { origin: "https://custom.test:8443", localStorage: [] };
  const source = { cookies: [], origins: [origin,
    { origin: "https://custom.test" }, { origin: "http://custom.test:8443" },
    { origin: "https://custom.test:8443/path" }], unknownSecrets: "excluded" };
  assert.deepEqual(scopeRuntimeState(source, "https://custom.test:8443/base"), { cookies: [], origins: [origin] });
  assert.deepEqual(scopeRuntimeState(null, "https://custom.test"), { cookies: [], origins: [] });
  for (const url of ["not a URL", "file:///tmp/state", "https://user:password@custom.test"]) {
    assert.throws(() => scopeRuntimeState(source, url));
  }
});

test("secure and partitioned cookies cannot broaden the export scope", () => {
  assert.equal(cookieMatchesOrigin({ domain: "localhost", secure: true }, runtimeStateOrigin("http://localhost")), false);
  const target = runtimeStateOrigin("https://www.udio.com");
  assert.equal(cookieMatchesOrigin({ domain: ".udio.com", partitionKey: "https://other.test" }, target), false);
  assert.equal(cookieMatchesOrigin({ domain: ".udio.com", partitionKey: target.origin }, target), true);
  assert.equal(cookieMatchesOrigin({ domain: ".0.0.1" }, runtimeStateOrigin("http://127.0.0.1")), false);
});
