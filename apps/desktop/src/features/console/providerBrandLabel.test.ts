import { describe, expect, it } from "vitest";

import { providerBrandLabel } from "./providerBrandLabel";

describe("providerBrandLabel", () => {
  it("names a provider after the site brand instead of its slug", () => {
    expect(providerBrandLabel({ providerId: "jina-reader", providerPreset: "jina-reader" })).toBe(
      "Jina Reader",
    );
    expect(providerBrandLabel({ providerId: "chataibot", providerPreset: "chataibot" })).toBe(
      "ChatAIBot",
    );
    expect(providerBrandLabel({ providerId: "you", providerPreset: "you" })).toBe("You.com");
    expect(providerBrandLabel({ providerId: "xfyun-maas", providerPreset: "openai" })).toBe(
      "iFlytek MaaS",
    );
  });

  it("drops the routing noise operators typed into the label", () => {
    // The config label is "Gemini Web /u/1/", which is a browser profile path,
    // not a brand.
    expect(
      providerBrandLabel({
        providerId: "gemini-web-secondary",
        providerPreset: "gemini-web-chat-modular",
        providerLabel: "Gemini Web /u/1/",
        vendorName: "Google / Gemini",
      }),
    ).toBe("Gemini Web · Secondary");
    expect(
      providerBrandLabel({
        providerId: "muyuan-openai",
        providerPreset: "muyuan-openai",
        providerLabel: "Muyuan · 第三方 OpenAI 兼容",
      }),
    ).toBe("Muyuan");
  });

  it("keeps the id apart from the brand when the id says more", () => {
    expect(
      providerBrandLabel({ providerId: "gemini-web-alpha", providerPreset: "gemini-web-chat" }),
    ).toBe("Gemini Web · Alpha");
  });

  it("prefers a hand-written label over a synthesized one for unknown providers", () => {
    expect(
      providerBrandLabel({
        providerId: "corp-gateway-1",
        providerPreset: "custom-openai-compatible",
        providerLabel: "Corp Gateway",
      }),
    ).toBe("Corp Gateway");
  });

  it("falls back to the vendor name, then to a tidied id", () => {
    expect(
      providerBrandLabel({
        providerId: "acme-relay",
        providerPreset: "custom-openai-compatible",
        vendorName: "Acme / Relay",
      }),
    ).toBe("Acme Relay");
    expect(
      providerBrandLabel({ providerId: "some-new-ai-api", providerPreset: "unknown-preset" }),
    ).toBe("Some New AI API");
  });

  it("gives every deployed provider a distinct label", () => {
    // Duplicate tab labels on the entitlement card back would be unreadable, so
    // the same-brand providers keep a qualifier.
    const deployed = [
      { providerId: "gemini-web-secondary", providerPreset: "gemini-web-chat-modular" },
      { providerId: "muyuan-openai", providerPreset: "muyuan-openai" },
      { providerId: "accio", providerPreset: "accio" },
      { providerId: "chataibot", providerPreset: "chataibot" },
      { providerId: "codex", providerPreset: "codex" },
      { providerId: "exa", providerPreset: "exa" },
      { providerId: "gemini-business", providerPreset: "gemini-business" },
      { providerId: "gemini-canvas", providerPreset: "gemini-canvas-program-relay" },
      { providerId: "gemini-canvas-chat", providerPreset: "gemini-canvas-chat" },
      { providerId: "jina-reader", providerPreset: "jina-reader" },
      { providerId: "jina-search", providerPreset: "jina-search" },
      { providerId: "linkup", providerPreset: "linkup" },
      { providerId: "longcat", providerPreset: "longcat-openai" },
      { providerId: "lumalabs", providerPreset: "lumalabs" },
      { providerId: "nvidia", providerPreset: "nvidia-openai" },
      { providerId: "openrouter", providerPreset: "openai" },
      { providerId: "poe", providerPreset: "poe-openai" },
      { providerId: "producer", providerPreset: "producer" },
      { providerId: "qwen-coding-plan-anthropic", providerPreset: "qwen-coding-plan-anthropic" },
      { providerId: "qwen-coding-plan-openai", providerPreset: "qwen-coding-plan-openai" },
      { providerId: "qwen-dashscope-openai", providerPreset: "qwen-dashscope-openai" },
      { providerId: "qwen-web-chat", providerPreset: "qwen-web-chat" },
      { providerId: "suno", providerPreset: "suno" },
      { providerId: "tavily", providerPreset: "tavily" },
      { providerId: "udio", providerPreset: "udio" },
      { providerId: "websearchapi", providerPreset: "websearchapi" },
      { providerId: "xfyun-maas", providerPreset: "openai" },
      { providerId: "you", providerPreset: "you" },
    ];

    const labels = deployed.map((provider) => providerBrandLabel(provider));

    expect(labels).toHaveLength(28);
    expect(new Set(labels).size).toBe(labels.length);
    expect(labels.every((label) => label.length > 0)).toBe(true);
    // No slug survives: nothing keeps a lowercase-hyphen shape.
    expect(labels.filter((label) => /^[a-z0-9-]+$/.test(label))).toEqual([]);
  });
});
