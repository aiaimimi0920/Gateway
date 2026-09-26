import assert from "node:assert/strict";
import test from "node:test";
import { normalizeGeminiBrowserAssetUrl, pushUniqueMediaUrl } from "../gemini-canvas-browser-pool-media-urls.mjs";

for (const domain of ["googleusercontent.com", "googlevideo.com", "gvt1.com"]) {
  test(`media HTTP upgrade recognizes ${domain} and real subdomains`, () => {
    for (const host of [domain, `files.${domain}`, `nested.files.${domain}`]) {
      assert.equal(normalizeGeminiBrowserAssetUrl(`  http://${host}/asset?sig=A%2Fb&x=1#part  `), `https://${host}/asset?sig=A%2Fb&x=1#part`);
    }
    assert.equal(normalizeGeminiBrowserAssetUrl(`HTTP://FILES.${domain.toUpperCase()}:8080/asset`), `https://files.${domain}:8080/asset`);
  });
}

test("media HTTP upgrade deduplicates captured insecure and secure variants", () => {
  for (const urls of [
    ["http://files.googleusercontent.com/asset.png", "https://files.googleusercontent.com/asset.png"],
    ["https://files.googleusercontent.com/asset.png", "http://files.googleusercontent.com/asset.png"],
  ]) {
    const store = [];
    pushUniqueMediaUrl(store, { url: urls[0], mimeType: "image/png", bodyBase64: "AQID" });
    pushUniqueMediaUrl(store, { url: urls[1], mimeType: "image/png", bodyBase64: "BAUG" });
    assert.deepEqual(store, [{ url: "https://files.googleusercontent.com/asset.png", mimeType: "image/png", bodyBase64: "AQID" }]);
  }
});

test("media HTTP upgrade leaves lookalike hosts and embedded domain text unchanged", () => {
  for (const domain of ["googleusercontent.com", "googlevideo.com", "gvt1.com"]) {
    for (const value of [
      `http://evil${domain}/asset`, `http://${domain}.fixture.invalid/asset`,
      `http://fixture.invalid/${domain}`, `http://fixture.invalid/?source=${domain}`,
      `http://${domain}@fixture.invalid/asset`, `http://${domain.replace(".", "x")}/asset`,
    ]) assert.equal(normalizeGeminiBrowserAssetUrl(value), value);
  }
});

test("media HTTP upgrade leaves other schemes and signed HTTPS bytes unchanged", () => {
  for (const value of [
    "https://files.googleusercontent.com/asset?sig=A%2fb+%2B&x=1#part",
    "blob:http://files.googleusercontent.com/id", "ftp://googlevideo.com/asset",
    "http://fixture.invalid/asset", "relative/asset", "not a URL",
  ]) assert.equal(normalizeGeminiBrowserAssetUrl(value), value);
});
