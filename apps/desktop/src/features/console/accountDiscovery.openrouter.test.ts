import { expect, it } from "vitest";
import { supportsAccountDiscovery } from "./accountDiscovery";

it("allows explicit discovery for legacy OpenRouter OpenAI presets", () => {
  for (const base_url of ["https://openrouter.ai/api", "https://openrouter.ai/api/",
    "https://openrouter.ai/api/v1", "https://openrouter.ai:443/api/v1/"]) {
    for (const adapter of [undefined, null, "openai_compatible"]) {
      expect(supportsAccountDiscovery({ preset: "openai", base_url, adapter })).toBe(true);
    }
  }
});

it("does not unlock unrelated presets or ambiguous OpenRouter addresses", () => {
  for (const base_url of ["http://openrouter.ai/api", "https://openrouter.ai.evil.test/api",
    "https://openrouter.ai:444/api", "https://user@openrouter.ai/api",
    "https://openrouter.ai/api?key=hidden", "https://openrouter.ai/api#fragment",
    "https://openrouter.ai/custom/api", "not a URL"]) {
    expect(supportsAccountDiscovery({ preset: "openai", base_url })).toBe(false);
  }
  expect(supportsAccountDiscovery({ preset: "other", base_url: "https://openrouter.ai/api" })).toBe(false);
  expect(supportsAccountDiscovery({ preset: "openai", adapter: "custom", base_url: "https://openrouter.ai/api" })).toBe(false);
  expect(supportsAccountDiscovery({ adapter: "openai_compatible" })).toBe(true);
});
