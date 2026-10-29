import { render } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { card, pilotSection, renderWorkspace, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";
import { ProviderBrandIcon } from "./ProviderBrandIcon";
import { providerBrandAssets } from "./providerBrandAssets";
import { providerCardVisual } from "./providerCardVisual";

const brands = [
  ["jina", "jina", "Jina AI", "jina"],
  ["exa", "exa", "Exa", "exa"],
  ["linkup", "linkup", "Linkup", "linkup"],
  ["you", "you", "You.com", "you"],
  ["websearchapi", "websearchapi", "WebSearchAPI", "websearchapi"],
  ["tavily", "tavily", "Tavily", "tavily"],
  ["openrouter-secondary", "openai", "OpenRouter", "openrouter"],
  ["chatgpt", "chatgpt-codex-oauth-official-api", "ChatGPT", "openai"],
] as const;

describe("provider trademark icons", () => {
  it.each([...providerBrandAssets.keys()])("uses a local image for the known %s brand", (iconKey) => {
    const { container } = render(<ProviderBrandIcon iconKey={iconKey} fallback="?" />);
    expect(container.querySelector("img")).toHaveAttribute("src", providerBrandAssets.get(iconKey));
    expect(container.querySelector("[data-provider-icon]")).toHaveAttribute("aria-hidden", "true");
  });

  it("retains an offline monogram for an unknown provider without inventing a trademark", () => {
    const { container } = render(<ProviderBrandIcon iconKey="generic" fallback="C" />);
    expect(container.querySelector("img")).toBeNull();
    expect(container.textContent).toBe("C");
  });

  it.each(brands)("renders the bundled %s trademark on both card faces", (id, preset, label, key) => {
    renderWorkspace(workspaceProps({ pilotSections: [pilotSection({
      providerId: id, providerIds: [id], providerPreset: preset,
      providerLabel: label, vendorLabel: label,
    })] }));
    for (const side of ["front", "back"]) {
      const icon = card(id).querySelector(`.nt-provider-card__${side} [data-provider-icon]`);
      expect(icon).toHaveAttribute("data-provider-icon", key);
      const mark = icon?.querySelector("img");
      expect(mark).not.toBeNull();
      expect(mark).toHaveAttribute("alt", "");
      expect(mark?.getAttribute("src")).not.toMatch(/^https?:/);
      expect(icon?.textContent).toBe("");
    }
  });

  it.each([
    ["custom-pool", "jina-reader", "Reader", "jina"],
    ["custom-pool", "openai", "OpenRouter", "openrouter"],
    ["custom-pool", "chatgpt-codex-oauth-official-api", "OAuth", "openai"],
    ["codex", "openai", "Codex", "openai"],
  ])("resolves the brand for %s / %s / %s", (providerId, providerPreset, providerLabel, iconKey) => {
    expect(providerCardVisual({ providerId, providerPreset, providerLabel }).iconKey).toBe(iconKey);
  });
});
