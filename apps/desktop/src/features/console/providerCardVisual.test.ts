import { describe, expect, it } from "vitest";

import {
  providerCardCategoryLabel,
  providerCardInitial,
  providerCardVisual,
} from "./providerCardVisual";

const t = (zh: string, _en: string) => zh;

describe("providerCardVisual", () => {
  it("classifies media and browser surfaces that the add-provider catalog does not template", () => {
    expect(
      providerCardVisual({
        providerId: "suno",
        providerLabel: "Suno",
        providerPreset: "suno",
      }).category,
    ).toBe("media");
    expect(
      providerCardVisual({
        providerId: "gemini-canvas",
        providerLabel: "Gemini Canvas",
        providerPreset: "gemini-canvas",
      }).category,
    ).toBe("media");
    expect(
      providerCardVisual({
        providerId: "gemini-canvas-chat",
        providerLabel: "Gemini Canvas Chat",
        providerPreset: "gemini-canvas-chat",
      }).category,
    ).toBe("browser");
    expect(
      providerCardVisual({
        providerId: "codex",
        providerLabel: "codex",
        providerPreset: "codex",
      }).category,
    ).toBe("browser");
  });

  it("keeps catalog-backed providers on their catalog category", () => {
    expect(
      providerCardVisual({
        providerId: "openrouter",
        providerLabel: "OpenRouter",
        providerPreset: "openai",
      }).category,
    ).toBe("aggregator");
    expect(
      providerCardVisual({
        providerId: "tavily",
        providerLabel: "Tavily",
        providerPreset: "tavily",
      }).category,
    ).toBe("search");
    expect(
      providerCardVisual({
        providerId: "nvidia",
        providerLabel: "NVIDIA",
        providerPreset: "nvidia-openai",
      }).category,
    ).toBe("mainstream");
  });

  it("routes third-party OpenAI-compatible providers through the catalog classifier", () => {
    expect(
      providerCardVisual({
        providerId: "muyuan-openai",
        providerLabel: "Muyuan",
        providerPreset: "muyuan-openai",
      }).category,
    ).toBe("third-party-compatible");
    expect(
      providerCardVisual({
        providerId: "some-vendor",
        providerLabel: "Some Vendor",
        providerPreset: null,
        adapter: "openai_compatible",
        protocolProfile: "openai_compatible_generic",
      }).category,
    ).toBe("third-party-compatible");
  });

  it("falls back to mainstream for providers with no known classification", () => {
    expect(
      providerCardVisual({
        providerId: "unknown-provider",
        providerLabel: "Unknown",
        providerPreset: "unknown-preset",
      }).category,
    ).toBe("mainstream");
  });

  it("maps known channels to local icon keys and keeps an offline fallback", () => {
    expect(
      providerCardVisual({
        providerId: "openai",
        providerLabel: "OpenAI",
        providerPreset: "openai",
      }),
    ).toMatchObject({ iconKey: "openai", iconLabel: "AI" });
    expect(
      providerCardVisual({
        providerId: "gemini-api",
        providerLabel: "Google Gemini",
        providerPreset: "gemini-api",
      }),
    ).toMatchObject({ iconKey: "google", iconLabel: "G" });
    expect(
      providerCardVisual({
        providerId: "longcat",
        providerLabel: "LongCat",
        providerPreset: "longcat-openai",
      }),
    ).toMatchObject({ iconKey: "longcat", iconLabel: "LC" });
    expect(
      providerCardVisual({
        providerId: "custom-provider",
        providerLabel: "Custom Provider",
        providerPreset: null,
      }),
    ).toMatchObject({ iconKey: "generic", iconLabel: "C" });
  });

  it("derives a single glyph from the label, ignoring punctuation and separators", () => {
    expect(providerCardInitial("OpenRouter", "openrouter")).toBe("O");
    expect(providerCardInitial("· Muyuan", "muyuan-openai")).toBe("M");
    expect(providerCardInitial("讯飞星辰", "xfyun-maas")).toBe("讯");
    expect(providerCardInitial("", "gemini-canvas")).toBe("G");
    expect(providerCardInitial("---", "___")).toBe("?");
  });

  it("labels every category so no card renders an untranslated category", () => {
    for (const category of [
      "mainstream",
      "aggregator",
      "search",
      "media",
      "browser",
      "third-party-compatible",
    ] as const) {
      expect(providerCardCategoryLabel(category, t)).not.toHaveLength(0);
    }
  });
});
