import { expect, it } from "vitest";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { duplicateCredentialDialogValue } from "./providerCredentialDraft";
import { providerTargetEditable } from "./providerConnectionDraft";
import { filterVercelAccountModels, registerVercelModels, VERCEL_CREDIT_AUTH_MODE, VERCEL_FREE_TEMPLATE, vercelModels } from "./vercelFreeCatalog";

it("defaults to zero-price models and adds metered routes without replacing other providers", () => {
  const [first] = vercelModels(false);
  const paid = vercelModels(true).at(-1)!;
  expect(VERCEL_FREE_TEMPLATE.supportedModels).toEqual(vercelModels(false));
  expect(VERCEL_FREE_TEMPLATE.supportedModels).not.toContain(paid);
  const legacy = {
    providers: [{ id: "vercel", preset: VERCEL_FREE_TEMPLATE.preset, supported_models: [first] }],
    model_routes: [{ pattern: paid, provider_ids: ["other"] }],
  } as ConsoleRouteDocument;
  registerVercelModels(legacy, "vercel", [...vercelModels(true), "unknown/model"]);
  registerVercelModels(legacy, "vercel", vercelModels(true));
  expect(legacy.providers[0]).toMatchObject({ supported_models: vercelModels(true) });
  expect(legacy.model_routes).toHaveLength(vercelModels(true).length);
  expect(legacy.model_routes[0]).toEqual({ pattern: paid, provider_ids: ["other", "vercel"] });
});

it("keeps Free Credit models separate from zero-price and out-of-tier models", () => {
  expect(vercelModels(false)).toHaveLength(2);
  expect(vercelModels(true)).toHaveLength(19);
  for (const model of ["openai/gpt-5-mini", "google/gemini-2.5-flash", "moonshotai/kimi-k2.7-code"]) {
    expect(vercelModels(true)).toContain(model);
    expect(vercelModels(false)).not.toContain(model);
  }
  expect(vercelModels(true)).not.toContain("openai/gpt-5.4-mini");
});

it("filters saved models by account tier without affecting other providers", () => {
  const zero = "recursive/flint-v1.0";
  const credit = "alibaba/qwen-3-14b";
  const retired = ["reflection/beam-free", "humansand/ampersand-v0.1", "stealth/glyph-cluster"];
  const models = [...retired, zero, credit];
  expect(filterVercelAccountModels("vercel-ai-gateway", undefined, models)).toEqual([zero]);
  expect(filterVercelAccountModels("vercel-ai-gateway", VERCEL_CREDIT_AUTH_MODE, models)).toEqual([zero, credit]);
  expect(filterVercelAccountModels("openai", undefined, models)).toEqual(models);
});

it("preserves monthly credit tier on duplicate and locks managed target URLs", () => {
  const credit = "openai/gpt-5-mini";
  const doc = {
    providers: [{
      id: "vercel", preset: "vercel-ai-gateway", base_url: "https://ai-gateway.vercel.sh/v1",
      credentials: [{ id: "team-a", auth_mode: VERCEL_CREDIT_AUTH_MODE, supported_models: [credit] }],
    }],
    model_routes: [], aliases: {},
  } as ConsoleRouteDocument;
  const duplicated = duplicateCredentialDialogValue(doc, "vercel", {
    accountId: "team-a", providerId: "vercel", displayName: "team-a", enabled: true,
  } as Parameters<typeof duplicateCredentialDialogValue>[2]);
  expect(duplicated.vercelMonthlyCredit).toBe(true);
  expect(duplicated.supportedModelsText).toBe(credit);
  expect(providerTargetEditable({ id: "vercel", preset: "vercel-ai-gateway" })).toBe(false);
  expect(providerTargetEditable({ id: "kilo-free", preset: "kilo-free" })).toBe(false);
});
