import { expect, it } from "vitest";
import { applySiteHealth, isCodexProvider, siteHealthConfigSchema } from "./siteHealth";
import { parseRouteDocument } from "./routeDocument";
import { credentialDialogValueFromDocument } from "./providerCredentialDraft";
import { updateExplicitCredential } from "./credentialDocument";

it("adds/removes independent site policy without changing credentials or the source draft", () => {
  const document = parseRouteDocument(JSON.stringify({ providers:[{ id:"site", base_url:"https://fixture.invalid/v1", credentials:[{id:"account",api_key:"fixture"}] }], model_routes:[], aliases:{} }));
  const config = siteHealthConfigSchema.parse({});
  expect(config.mode).toBe("advisory"); expect(config.unknown_policy).toBe("allow");
  const next = applySiteHealth(document, "site", config);
  expect(next.providers[0]).toEqual({ ...(document.providers[0] as object), site_health:config });
  expect(document.providers[0]).not.toHaveProperty("site_health");
  expect(applySiteHealth(next,"site",null)).toEqual(document);
});

it("recognizes only native Codex and preserves explicit true/false/inherit credential choices", () => {
  expect(isCodexProvider({base_url:"https://chatgpt.com/backend-api/codex/"})).toBe(true);
  expect(isCodexProvider({base_url:"https://chatgpt.com.attacker.invalid/backend-api/codex"})).toBe(false);
  expect(isCodexProvider({preset:"chatgpt-codex-oauth-official-api"})).toBe(true);
  expect(isCodexProvider({preset:"chatgpt-codex-oauth-official-api",base_url:"https://fixture.invalid/v1"})).toBe(false);
  const document = parseRouteDocument(JSON.stringify({ providers:[{ id:"site", credentials:[{id:"account",codex_basispoints_enabled:true}] }], model_routes:[], aliases:{} }));
  expect(credentialDialogValueFromDocument(document,"site","account")?.codexBasispointsEnabled).toBe(true);
  const native = updateExplicitCredential(document,{providerId:"site",credentialId:"account"},{codex_basispoints_enabled:false});
  expect(credentialDialogValueFromDocument(native,"site","account")?.codexBasispointsEnabled).toBe(false);
  const inherited = updateExplicitCredential(native,{providerId:"site",credentialId:"account"},{codex_basispoints_enabled:undefined});
  expect(JSON.stringify(inherited)).not.toContain("codex_basispoints_enabled");
});
