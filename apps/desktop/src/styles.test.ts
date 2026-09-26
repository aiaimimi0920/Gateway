import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Follow the actual flat import entry so these assertions cover the loaded cascade.
const sourceRoot = resolve(process.cwd(), "src");
const styles = readFileSync(resolve(sourceRoot, "styles.css"), "utf8").replace(
  /^@import "(\.\/styles\/[a-z-]+\.css)";\n/gm,
  (_statement: string, relative: string) => {
    const owner = readFileSync(resolve(sourceRoot, relative), "utf8");
    if (owner.includes("@import")) throw new Error("Nested stylesheet import is not covered");
    return owner;
  },
);
if (styles.includes("@import")) throw new Error("Unsupported stylesheet import");

describe("Gateway Neuro theme contract", () => {
  it("defines the canonical palette once and exposes semantic roles", () => {
    expect(styles).toContain("--nt-color-signal-yellow: #d9ff38;");
    expect(styles).toContain("--nt-color-signal-green: #22c55e;");
    expect(styles).toContain("--nt-color-info-blue: #06b6d4;");
    expect(styles).toContain("--nt-color-danger-red: #f43f5e;");
    expect(styles).toContain("--nt-color-background: #06080d;");
    expect(styles).toContain("--nt-color-surface: #090c11;");
    expect(styles).toContain("--nt-color-panel: #0e1218;");
    expect(styles).toContain("--nt-color-control: #111720;");
    expect(styles).toContain("--nt-color-control-hover: #18222c;");
    expect(styles).toContain("--nt-color-text: #f7f8ef;");
    expect(styles).toContain("--nt-color-text-muted: #929a9f;");
    expect(styles).toContain("--nt-success: var(--nt-color-signal-green);");
    expect(styles).toContain("--nt-info: var(--nt-color-info-blue);");
    expect(styles).toContain("--nt-danger: var(--nt-color-danger-red);");
  });

  it("does not restore legacy purple accents or decorative radial gradients", () => {
    expect(styles).not.toMatch(/--nt-(?:violet|fuchsia)\s*:/i);
    expect(styles).not.toMatch(/#(?:8b5cf6|d946ef|8dde12)\b/i);
    expect(styles).not.toContain("radial-gradient(");
  });

  it("keeps status components bound to semantic aliases", () => {
    expect(styles).toMatch(/\.nt-badge--success\s*\{[^}]*var\(--nt-success-soft\)/s);
    expect(styles).toMatch(/\.nt-badge--warning\s*\{[^}]*var\(--nt-warning-soft\)/s);
    expect(styles).toMatch(/\.nt-badge--danger\s*\{[^}]*var\(--nt-danger-soft\)/s);
    expect(styles).toMatch(/\.nt-alert--success\s*\{[^}]*var\(--nt-success-soft\)/s);
    expect(styles).toMatch(/\.nt-alert--danger\s*\{[^}]*var\(--nt-danger-soft\)/s);
    expect(styles).toMatch(
      /\.nt-filter-count\s*\{[^}]*border: 1px solid var\(--nt-border-strong\);[^}]*border-radius: var\(--nt-radius-md\);[^}]*background: var\(--nt-signal-soft\);/s,
    );
  });

  it("loads the canonical layer after the base responsive rules", () => {
    const baseMobileIndex = styles.indexOf("@media (max-width: 720px)");
    const canonicalLayerIndex = styles.indexOf("/* Canonical Neuro theme layer.");

    expect(baseMobileIndex).toBeGreaterThan(-1);
    expect(canonicalLayerIndex).toBeGreaterThan(baseMobileIndex);
    expect(styles).toContain("@media (min-width: 721px) and (max-width: 1200px)");
    expect(styles).toContain(".nt-provider-account-table {\n    min-width: 980px;");
  });

  it("uses flippable entitlement cards with attached account expansion", () => {
    expect(styles).toMatch(
      /\.nt-entitlement-group-grid\s*\{[^}]*grid-template-columns: repeat\(auto-fill, minmax\(min\(100%, 300px\), 1fr\)\);/,
    );
    expect(styles).toMatch(
      /\.nt-entitlement-group-card--flipped \.nt-entitlement-group-card__inner\s*\{[^}]*transform: rotateY\(180deg\);/,
    );
    expect(styles).toMatch(
      /\.nt-entitlement-group-card__face\s*\{[^}]*backface-visibility: hidden;/,
    );
    expect(styles).toMatch(
      /\.nt-entitlement-group-card__front-body,[\s\S]*\.nt-entitlement-group-card__back-body\s*\{[^}]*overflow-y: auto;[^}]*overscroll-behavior: contain;[^}]*scrollbar-gutter: stable;/,
    );
    expect(styles).toMatch(
      /\.nt-entitlement-group-card__accounts--attached\s*\{[^}]*grid-column: 1 \/ -1;/,
    );
    expect(styles).toMatch(
      /@media \(min-width: 1201px\)[\s\S]*\.nt-group-members\s*\{[^}]*max-height: min\(56vh, 620px\);[^}]*overflow: auto;/,
    );
    expect(styles).toMatch(
      /\.nt-group-panel--members\s*\{[^}]*border-top: 1px solid var\(--nt-border\);[^}]*background: transparent;/,
    );
  });

  it("uses compact semantic tooltips and a visually hidden workspace heading", () => {
    expect(styles).toMatch(
      /\.nt-tooltip\s*\{[^}]*background: var\(--nt-color-focus-surface\);[^}]*color: var\(--nt-color-focus-ink\);/,
    );
    expect(styles).toMatch(
      /\.nt-visually-hidden\s*\{[^}]*width: 1px;[^}]*clip-path: inset\(50%\);/,
    );
    expect(styles).toMatch(/\.nt-workspace-toolbar\s*\{[^}]*padding: 14px 16px;/);
  });

  it("keeps draft actions reachable without making the mobile bar sticky", () => {
    expect(styles).toMatch(
      /\.nt-workspace-action-bar\s*\{[^}]*position: sticky;[^}]*top: 0;[^}]*background: color-mix\(in srgb, var\(--nt-color-panel\) 94%, transparent\);/,
    );
    expect(styles).toMatch(
      /@media \(max-width: 720px\)[\s\S]*\.nt-workspace-action-bar\s*\{[^}]*position: static;[^}]*flex-direction: column;/,
    );
  });

  it("uses compact controls and bounded responsive navigation", () => {
    expect(styles).toMatch(
      /\.nt-input,[\s\S]*\.nt-textarea\s*\{[^}]*border-radius: var\(--nt-radius-md\);[^}]*padding: 10px 12px;[^}]*background: var\(--nt-color-control\);/,
    );
    expect(styles).toMatch(
      /\.nt-entitlement-account-grid\s*\{[^}]*grid-template-columns: repeat\(auto-fill, 240px\);[^}]*justify-content: start;/,
    );
    expect(styles).toMatch(
      /@media \(max-width: 720px\)[\s\S]*\.nt-rail__nav\s*\{[^}]*display: flex;[^}]*overflow-x: auto;/,
    );
    expect(styles).toMatch(
      /\.nt-shell\s*\{[^}]*--nt-shell-rail-width: 186px;[^}]*grid-template-columns: var\(--nt-shell-rail-width\) minmax\(0, 1fr\);/,
    );
    expect(styles).toMatch(
      /\.nt-shell--rail-collapsed,\s*\n\.nt-shell--console-rail-collapsed\s*\{[^}]*--nt-shell-rail-width: 52px;/,
    );
    expect(styles).toMatch(
      /@media \(max-width: 720px\)[\s\S]*\.nt-console-rail-toggle\s*\{[^}]*display: none;/,
    );
  });

});
