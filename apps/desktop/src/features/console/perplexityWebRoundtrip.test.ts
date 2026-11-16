import { writeFileSync } from "node:fs";
import { expect, it } from "vitest";
import { parsePerplexityWebMaterial } from "./perplexityWebMaterial";
import { PERPLEXITY_SESSION_COOKIE as NAME } from "./perplexityWebCookies";

// Opt-in synthetic export feeds the actual Rust secret resolver; never a browser or live credential.
const output = process.env.GATEWAY_PERPLEXITY_MATERIAL_FIXTURE_PATH;
it.skipIf(!output)("exports normalized synthetic Perplexity materials for backend roundtrip", () => {
  const base = { product: "perplexity-web", userAgent: "synthetic-roundtrip-agent" };
  const cookies = (name: string, value: string, expires: number) => ({ name, value, expires, domain: ".perplexity.ai", path: "/", secure: true, httpOnly: true, sameSite: "Lax" });
  const expiry = Date.parse("2099-01-01T00:00:00Z") / 1000;
  const fixtures = [
    { ...base, token: "synthetic-explicit", cfClearance: "synthetic-clearance" },
    { ...base, cookieHeader: `unrelated=not-forwarded; ${NAME}=synthetic-header`, expiresAt: "2099-01-01T00:00:00Z" },
    { ...base, storageState: { cookies: [cookies(`${NAME}.1`, "chunks", expiry + 600), cookies(`${NAME}.0`, "synthetic-", expiry), cookies("cf_clearance", "synthetic-clearance", expiry + 1200)], origins: [] } },
  ].map((input) => parsePerplexityWebMaterial(JSON.stringify(input)));
  expect(fixtures.map((m) => m.token)).toEqual(["synthetic-explicit", "synthetic-header", "synthetic-chunks"]);
  expect(fixtures[2].expiresAt).toBe("2099-01-01T00:00:00.000Z");
  writeFileSync(output!, JSON.stringify(fixtures, null, 2), "utf8");
});
