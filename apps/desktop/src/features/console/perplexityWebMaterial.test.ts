import { describe, expect, it } from "vitest";
import { parsePerplexityWebMaterial } from "./perplexityWebMaterial";
import { PERPLEXITY_SESSION_COOKIE as NAME } from "./perplexityWebCookies";

const NOW = Date.parse("2026-11-03T00:00:00Z");
const base = { product: "perplexity-web", userAgent: "synthetic-agent" };
const cookie = (name = NAME, value = "synthetic-session", extra = {}) => ({ name, value, domain: ".perplexity.ai", path: "/", secure: true, httpOnly: true, sameSite: "Lax", expires: NOW / 1000 + 3600, ...extra });
const parse = (value: unknown) => parsePerplexityWebMaterial(JSON.stringify({ ...base, ...value as object }), NOW);
const storage = (cookies: unknown[], extra = {}) => ({ storageState: { cookies, origins: [{ origin: "https://attacker.invalid", localStorage: [{ name: "token", value: "never-forward" }] }] }, ...extra });

describe("Perplexity Web material boundary", () => {
  it("normalizes explicit tokens without treating their shape as proof of login", () => {
    const result = parse({ token: "opaque-session", cfClearance: "opaque-clearance" });
    expect(result.cookieHeader).toBe(`${NAME}=opaque-session; cf_clearance=opaque-clearance`);
    expect(result.expiresAt).toBeUndefined();
    expect(parse(result)).toEqual(result);
  });
  it("projects only fixed-site cookies, not full profile or localStorage", () => {
    const result = parse(storage([cookie(), cookie("cf_clearance", "synthetic-clearance"), cookie("tracking", "never-forward"), cookie(NAME, "foreign", { domain: "attacker.invalid" })]));
    expect(result.cookieHeader).toBe(`${NAME}=synthetic-session; cf_clearance=synthetic-clearance`);
    expect(JSON.stringify(result)).not.toMatch(/never-forward|foreign|localStorage|attacker/);
    expect(result.expiresAt).toBe("2026-11-03T01:00:00.000Z");
  });
  it("assembles contiguous browser chunks in index order without decoding and narrows expiry", () => {
    const result = parse(storage([cookie(`${NAME}.1`, "second%2F", { expires: NOW / 1000 + 600 }), cookie(`${NAME}.0`, "first")], { expiresAt: "2026-11-03T00:05:00Z" }));
    expect(result.token).toBe("firstsecond%2F");
    expect(result.expiresAt).toBe("2026-11-03T00:05:00.000Z");
  });
  it.each([
    [cookie(NAME, "x", { secure: false })], [cookie(NAME, "x", { path: "/rest" })],
    [cookie(NAME, "x", { domain: "perplexity.ai" })], [cookie(NAME, "x", { domain: "www.perplexity.ai.attacker.invalid" })],
    [cookie(NAME, "x", { expires: NOW / 1000 })], [cookie(NAME, "x", { expires: 0 })],
    [cookie(NAME, "x", { partitionKey: "https://attacker.invalid" })], [cookie(NAME, "x", { expires: undefined })],
    [cookie(), cookie()], [cookie(`${NAME}.1`)], [cookie(), cookie(`${NAME}.0`)],
    [cookie(`${NAME}.00`)], [cookie(`${NAME}.0`), cookie(`${NAME}.2`)], [cookie("cf_clearance")],
  ])("rejects untrusted browser cookie selection %#", (...cookies) => {
    expect(() => parse(storage(cookies))).toThrow(/^会话材料/);
  });
  it("accepts a host-only www session and unknown session-cookie expiry", () => {
    expect(parse(storage([cookie(NAME, "x", { domain: "www.perplexity.ai", expires: -1 })])).expiresAt).toBeUndefined();
  });
  it("strips unrelated raw-header cookies but refuses duplicate identities or unverified chunks", () => {
    expect(parse({ cookieHeader: `tracking=no; ${NAME}=x; cf_clearance=y` }).cookieHeader).toBe(`${NAME}=x; cf_clearance=y`);
    for (const cookieHeader of [`${NAME}=x; ${NAME}=x`, `${NAME}.0=x`, `${NAME}=x\r\nHost: leak`, "cf_clearance=y"]) {
      expect(() => parse({ cookieHeader })).toThrow(/^会话材料/);
    }
    expect(() => parse({ token: "other", cookieHeader: `${NAME}=x` })).toThrow(/^会话材料/);
  });
  it.each([
    { token: "private;injection" }, { token: "private\nvalue" }, { token: "x", userAgent: "bad\nagent" },
    { token: "x", userAgent: " " }, { token: "x", userAgent: "x".repeat(1025) }, { token: "x".repeat(16385) },
    { token: "x", product: "perplexity-api" }, { token: "x", apiKey: "private-api" },
    { token: "x", base_url: "https://attacker.invalid" }, { token: "x", refresh_token: "private-refresh" },
    { token: "x", expiresAt: "2026-02-30T00:00:00Z" }, { token: "x", expiresAt: "2026-11-03T00:00:00Z" },
  ])("rejects foreign/malformed material without echoing secrets %#", (value) => {
    try { parse(value); throw new Error("expected rejection"); }
    catch (error) { expect(String(error)).toMatch(/会话材料/); expect(String(error)).not.toMatch(/private|attacker/); }
  });
  it("bounds file/cookie/chunk counts and rejects mixed storage/header modes", () => {
    expect(() => parse({ token: "x", unused: "x".repeat(128 * 1024) })).toThrow(/128 KiB/);
    expect(() => parse(storage(Array.from({ length: 257 }, () => cookie())))).toThrow(/^会话材料/);
    expect(() => parse(storage(Array.from({ length: 17 }, (_, i) => cookie(`${NAME}.${i}`))))).toThrow(/^会话材料/);
    expect(() => parse(storage([cookie()], { token: "x" }))).toThrow(/^会话材料/);
    expect(() => parse(storage([cookie(`${NAME}.0`, "x".repeat(16000)), cookie(`${NAME}.1`, "x".repeat(16000))]))).toThrow(/^会话材料/);
  });
});
