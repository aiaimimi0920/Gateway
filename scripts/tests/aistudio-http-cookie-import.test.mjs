import assert from "node:assert/strict";
import { test } from "node:test";
import { cookieBundle, accountIndex, TARGET_URL } from "../import-aistudio-http-cookies.mjs";

const cookie = (name, extra = {}) => ({ name, value: "fixture-secret", domain: ".google.com",
  path: "/", expires: -1, secure: true, httpOnly: true, sameSite: "Lax", ...extra });

test("AI Studio import targets Playground and recognizes numeric account scope", () => {
  assert.equal(TARGET_URL, "https://aistudio.google.com/prompts/new_chat");
  assert.equal(accountIndex("https://aistudio.google.com/u/2/prompts/new_chat"), "2");
  assert.equal(accountIndex(TARGET_URL), "0");
  assert.throws(() => accountIndex("https://evil.example/prompts/new_chat"));
});

test("cookie import preserves scopes and expiry without exporting browser storage", () => {
  const seed = cookie("SAPISID", { path: "/$rpc", expires: 4000000000 });
  const bundle = cookieBundle([seed, cookie("__Secure-1PAPISID"), cookie("__Secure-3PAPISID")], "2");
  assert.deepEqual(bundle.cookies[0], seed);
  assert.equal(bundle.authUser, "2");
  assert.equal(bundle.provider, "aistudio-web");
  assert.equal(bundle.origins, undefined);
  assert.equal(bundle.localStorage, undefined);
  assert.equal(bundle.storageState, undefined);
});

test("cookie import rejects invalid signing material and never imports unrelated sites", () => {
  const auth = [cookie("SAPISID"), cookie("__Secure-1PAPISID"), cookie("__Secure-3PAPISID")];
  assert.equal(cookieBundle([...auth, cookie("foreign", { domain: "evil.example" })], "0").cookies.length, 3);
  assert.throws(() => cookieBundle([], "0"));
  assert.throws(() => cookieBundle([cookie("SAPISID", { value: "secret\r\ninjected" })], "0"));
  assert.throws(() => cookieBundle(auth, "not-an-account"));
  assert.throws(() => cookieBundle([...auth, ...Array.from({ length: 65 }, (_, i) => cookie(`cookie${i}`))], "0"));
});
