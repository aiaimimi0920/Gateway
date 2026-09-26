import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { resolveUdioAccessToken, extractSupabaseAccessTokenFromCookies } from "../udio-browser/auth.mjs";

const authName = "sb-ssr-production-auth-token";
const cookie = (domain, token) => ({ domain, name: authName, value: JSON.stringify({ access_token: token }) });

test("auth fallback cannot use another site's same-named cookie", async () => {
  const context = {
    cookies: async (urls) => urls ? [] : [cookie("other.test", "foreign")],
  };
  const page = { context: () => context, evaluate: async () => null };
  assert.equal(await resolveUdioAccessToken(page, "https://www.udio.com"), null);
});

test("cookie chunk names must be exact and indices must be contiguous", () => {
  const value = JSON.stringify({ access_token: "synthetic" });
  for (const name of [`${authName}-foreign.0`, `${authName}.0suffix`, `${authName}.2`, `${authName}.01`]) {
    assert.equal(extractSupabaseAccessTokenFromCookies([{ name, value }]), null, name);
  }
  assert.equal(extractSupabaseAccessTokenFromCookies([
    { name: `${authName}.1`, value: value.slice(10) }, { name: `${authName}.0`, value: value.slice(0, 10) },
  ]), "synthetic");
});

test("same-provider cookie remains usable when the shared jar includes foreign auth", async () => {
  const page = {
    context: () => ({ cookies: async (urls) => urls ? []
      : [cookie("other.test", "foreign"), cookie(".udio.com", "provider")] }),
    evaluate: async () => { throw new Error("Unexpected localStorage read"); },
  };
  assert.equal(await resolveUdioAccessToken(page, "https://www.udio.com"), "provider");
});

test("localStorage fallback verifies origin inside the browser callback", async () => {
  for (const origin of ["https://other.test", "https://www.udio.com"]) {
    let reads = 0;
    const page = {
      context: () => ({ cookies: async () => [], newCDPSession: async () => null }),
      evaluate: async (callback, expectedOrigin) => vm.runInNewContext(
        `(${callback.toString()})(expectedOrigin)`, {
          expectedOrigin, window: { location: { origin } },
          localStorage: { length: 1, key: () => "auth",
            getItem: () => { reads += 1; return '{"access_token":"synthetic"}'; } },
        }),
    };
    const sameOrigin = origin === "https://www.udio.com";
    assert.equal(await resolveUdioAccessToken(page, "https://www.udio.com"), sameOrigin ? "synthetic" : null);
    assert.equal(reads, sameOrigin ? 1 : 0);
  }
});
