import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Follow the actual flat import entry so these assertions cover the loaded cascade.
const sourceRoot = resolve(process.cwd(), "src");
const uiStyles = readFileSync(new URL(import.meta.resolve("@neuro/ui/styles.css")), "utf8");
const accountControlStyles = readFileSync(resolve(sourceRoot, "styles/account-workspace-controls.css"), "utf8");
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
  it("maps application roles to the standard Provider palette without copying it", () => {
    const foundation = readFileSync(resolve(sourceRoot, "styles/foundation.css"), "utf8");
    const providerAliases = foundation.match(/\.neuro-root\s*\{([^}]+)\}/)?.[1];
    expect(providerAliases).toBeDefined();
    for (const [application, standard] of [["color-background", "bg"], ["color-surface", "surface"], ["color-panel", "panel"], ["color-control", "control"], ["color-control-hover", "control-hover"], ["text", "text"], ["text-muted", "muted"], ["signal", "signal"], ["success", "success"], ["info", "info"], ["danger", "danger"], ["border", "line"], ["rail", "rail"], ["hover", "hover"], ["shadow-surface", "shadow"]]) {
      expect(providerAliases).toContain(`--nt-${application}: var(--n-${standard});`);
    }
    expect(foundation).not.toMatch(/#[a-f\d]{3,8}\b/i);
    expect(foundation.match(/:root\s*\{([^}]+)\}/)?.[1]).not.toMatch(/var\(--n-/);
    expect(foundation).toContain("background: Canvas;");
    expect(styles).not.toMatch(/--nt-color-(?:signal|info|danger|text|rail|shadow)/);
  });

  it("does not restore legacy purple accents or decorative radial gradients", () => {
    expect(styles).not.toMatch(/--nt-(?:violet|fuchsia)\s*:/i);
    expect(styles).not.toMatch(/#(?:8b5cf6|d946ef|8dde12)\b/i);
    expect(styles).not.toContain("radial-gradient(");
  });

  it("leaves standard focus, reduced motion and group-picker appearance in the package", () => {
    const picker = readFileSync(resolve(sourceRoot, "styles/account-routing-pool-select.css"), "utf8");
    expect(picker).toMatch(/width: 112px;[\s\S]*min-width: 0;/);
    expect(picker).not.toMatch(/background:|color:|font-size:|outline:|opacity:|padding:|cursor:/);
    expect(styles).not.toContain(".nt-key-manager :focus-visible");
    expect(styles).not.toMatch(/button:focus-visible,\s*input:focus-visible/);
    expect(styles).not.toMatch(/animation-duration: 0\.01ms|scroll-behavior: auto !important/);
    expect(uiStyles).toContain("@media (forced-colors: active)");
    expect(uiStyles).toContain("@media (prefers-reduced-motion: reduce)");
    expect(styles).not.toContain(".nt-pilot-menu");
    expect(styles).not.toContain(".nt-provider-account-card__stats");
    expect(styles).not.toMatch(/\.nt-entitlement-group-card__action(?:[\s:{.\[-]|$)/);
  });

  it("keeps status components bound to semantic aliases", () => {
    expect(uiStyles).toMatch(/\.n-tone-success\s*\{[^}]*--n-tone: var\(--n-success\)/);
    expect(uiStyles).toMatch(/\.n-tone-warning\s*\{[^}]*--n-tone: var\(--n-signal\)/);
    expect(uiStyles).toMatch(/\.n-tone-danger\s*\{[^}]*--n-tone: var\(--n-danger\)/);
    expect(styles).not.toMatch(/\.nt-alert(?:[\s.{:#-]|$)|\.nt-validation-list(?:[\s.{:#-]|$)/);
    expect(uiStyles).toMatch(/\.n-notice\s*\{[^}]*border-left: 2px solid var\(--n-tone\)/);
  });

  it("loads application surface rules after the base responsive rules", () => {
    const baseMobileIndex = styles.indexOf("@media (max-width: 720px)");
    const canonicalLayerIndex = styles.indexOf("/* Application shell and card surfaces");

    expect(baseMobileIndex).toBeGreaterThan(-1);
    expect(canonicalLayerIndex).toBeGreaterThan(baseMobileIndex);
    expect(styles).not.toContain(".nt-provider-account-table");
    expect(uiStyles).toMatch(/\.n-table-scroll\s*\{[^}]*overflow: auto;[^}]*overscroll-behavior: contain;/);
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
    expect(uiStyles).toMatch(/\.n-table thead th\s*\{[^}]*position: sticky;[^}]*background: var\(--n-surface\);/);
  });

  it("uses compact semantic tooltips and a visually hidden workspace heading", () => {
    expect(uiStyles).toMatch(/\.n-tooltip\s*\{[^}]*color: var\(--n-tooltip-ink\);[^}]*background: var\(--n-tooltip-bg\);/);
    expect(styles).toMatch(
      /\.nt-visually-hidden\s*\{[^}]*width: 1px;[^}]*clip-path: inset\(50%\);/,
    );
    expect(uiStyles).toMatch(/\.n-toolbar\s*\{[^}]*display: flex;[^}]*flex-wrap: wrap;/);
  });

  it("keeps the current autosave status and standard actions reachable", () => {
    expect(styles).toMatch(/\.nt-console-autosave\s*\{[^}]*min-height: 36px;[^}]*background: transparent;/);
    expect(accountControlStyles).toMatch(/\.nt-pool-card-actions > :is\(\.n-button, \.n-switch\)\s*\{[^}]*min-width: 0;/);
  });

  it("uses compact controls and bounded responsive navigation", () => {
    expect(uiStyles).toMatch(/\.n-input\s*\{[^}]*min-height: 36px;[^}]*padding: 10px 12px;[^}]*background: var\(--n-control\);/);
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

  it("keeps both provider-pool headers at the front's fixed height and surface", () => {
    expect(styles).toContain("--nt-provider-card-head-height: 61px;");
    for (const face of ["front", "back"]) {
      expect(styles).toMatch(new RegExp(
        `\\.nt-provider-card__${face}\\s*\\{[^}]*grid-template-rows: var\\(--nt-provider-card-head-height\\) minmax\\(0, 1fr\\)`,
      ));
    }
    expect(styles).toMatch(
      /\.nt-provider-card__front-head,\s*\.nt-provider-card__back-head\s*\{[^}]*min-height: 0;[^}]*border-bottom: 1px solid var\(--nt-border\);[^}]*padding: 11px 12px;[^}]*background: var\(--nt-color-surface\);/,
    );
    expect(styles.match(/\.nt-provider-card__back-head\b/g)).toHaveLength(1);
    expect(styles).toMatch(
      /\.nt-provider-card__back-body \.nt-provider-lifecycle__group:first-child\s*\{[^}]*border-top: 0;/,
    );
  });

  it("truncates long provider titles on both faces without growing the header", () => {
    expect(/\.nt-provider-card__inner\s*\{[^}]*grid-template-columns: minmax\(0, 1fr\);/.test(styles)).toBe(true);
    expect(accountControlStyles).toMatch(/\.nt-pool-card-title\s*\{[^}]*min-width: 0;[^}]*max-width: 100%;/);
    expect(styles).toMatch(/\.nt-provider-card__back-title\s*\{[^}]*min-width: 0;[^}]*max-width: 100%;/);
    expect(accountControlStyles).toMatch(/\.nt-pool-card-title__text strong\s*\{[^}]*overflow: hidden;[^}]*text-overflow: ellipsis;[^}]*white-space: nowrap;/);
    expect(styles).toMatch(/\.nt-provider-card__back-title strong\s*\{[^}]*overflow: hidden;[^}]*text-overflow: ellipsis;[^}]*white-space: nowrap;/);
  });

  it("starts the progress section directly below the fixed front header", () => {
    expect(styles).toMatch(/\.nt-provider-card__front\s*\{[^}]*gap: 0;/);
    expect(styles).toMatch(/\.nt-provider-card__front-body\s*\{[^}]*padding: 0 12px 12px;/);
  });

  it("uses the shared routing-pool popup colors instead of native option highlighting", () => {
    expect(uiStyles).toMatch(/\.n-popup-select\s*\{[^}]*color: var\(--n-text\);[^}]*background: var\(--n-panel\);/);
    expect(uiStyles).toMatch(/\.n-popup-select-option\[data-highlighted\]\s*\{[^}]*background: var\(--n-hover\);/);
    expect(uiStyles).toMatch(/\.n-popup-select-option\[data-state="checked"\]\s*\{[^}]*color: var\(--n-signal\);[^}]*background: var\(--n-signal-soft\);/);
  });

  it("keeps provider pools as narrow as account cards instead of widening with the window", () => {
    expect(styles).toMatch(/\.nt-provider-card-grid\s*\{[^}]*--nt-provider-grid-card-width: 292px;[^}]*grid-template-columns: repeat\(var\(--nt-provider-grid-columns, auto-fill\), minmax\(min\(100%, var\(--nt-provider-grid-card-width\)\), var\(--nt-provider-grid-card-width\)\)\);[^}]*justify-content: safe center;/);
    expect(styles).toMatch(/\.nt-provider-card\s*\{[^}]*max-width: 292px;/);
    expect(styles).toMatch(/\.nt-provider-account-card\s*\{[^}]*width: 292px;/);
  });

  it("shares the provider face grid track and fits client controls beside site health", () => {
    expect(styles).toMatch(/\.nt-provider-card__face\s*\{[^}]*grid-area: 1 \/ 1;[^}]*min-height: 100%;/);
    expect(styles).toMatch(/\.nt-provider-lifecycle__pair,[\s\S]*?\.nt-provider-lifecycle__metrics\s*\{[^}]*grid-template-columns: repeat\(2, minmax\(0, 1fr\)\);/);
    expect(styles).toMatch(/\.nt-provider-lifecycle__client-row \.n-field\s*\{[^}]*display: flex;[^}]*align-items: center;/);
    expect(styles).toMatch(/\.nt-provider-lifecycle__client-row \.n-select\s*\{[^}]*flex: 1;[^}]*min-width: 0;/);
  });

  it("does not retain retired primitive skins or their toast animations", () => {
    expect(styles).not.toMatch(/\.(?:nt-btn|nt-input|nt-select|nt-textarea|nt-badge|nt-chip|nt-switch|nt-toast)(?:[\s.:>#\[,-]|\{)/);
    expect(styles).not.toMatch(/@keyframes nt-toast-(?:enter|exit)/);
  });
});
