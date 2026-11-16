import { expect, it } from "vitest";
import { providerAccessKind, providerAccessLabel } from "./providerAccessKind";
import { providerPoolEntries } from "./providerPoolView";
import { pilotSection } from "./AccountsLedgerWorkspace.fixtures";

it("classifies protocol metadata before labels and appends only one suffix", () => {
  expect(providerAccessKind({ providerPreset: "chatgpt-codex-oauth-official-api" })).toBe("oauth");
  expect(providerAccessKind({ providerPreset: "gemini-api", providerLabel: "My web" })).toBe("api");
  expect(providerAccessKind({ providerPreset: "gemini-web-chat" })).toBe("web");
  expect(providerAccessKind({ adapter: "gemini_business" })).toBe("web");
  expect(providerAccessLabel({ providerLabel: "chatgpt", providerPreset: "chatgpt-codex-oauth-official-api" })).toBe("codex-oauth");
  expect(providerAccessLabel({ providerLabel: "anthropic-api", providerPreset: "anthropic" })).toBe("anthropic-api");
  expect(providerAccessLabel({ providerLabel: "cohere", providerPreset: "cohere-chat" })).toBe("cohere-api");
  expect(providerAccessLabel({ providerLabel: "custom-web", providerPreset: "openai" })).toBe("custom-api");
});
it("combines kind selection with search and never mutates providers", () => {
  const sections = [pilotSection({ providerId: "oauth", providerPreset: "chatgpt-codex-oauth-official-api" }),
    pilotSection({ providerId: "api", providerPreset: "anthropic" }),
    pilotSection({ providerId: "web", providerPreset: "gemini-web-chat" })];
  const view = { query: "", sort: "default" as const };
  expect(providerPoolEntries(sections, view)).toHaveLength(3);
  expect(providerPoolEntries(sections, { ...view, accessKinds: [] })).toEqual([]);
  expect(providerPoolEntries(sections, { ...view, accessKinds: ["oauth", "web"] }).map(({ section }) => section.providerId)).toEqual(["oauth", "web"]);
  expect(providerPoolEntries(sections, { ...view, accessKinds: ["api"] }).map(({ section }) => section.providerId)).toEqual(["api"]);
  expect(providerPoolEntries(sections, { ...view, query: "missing", accessKinds: ["api"] })).toEqual([]);
  expect(sections.map(({ providerId }) => providerId)).toEqual(["oauth", "api", "web"]);
});
it("keeps the native Kiro Key in API despite its shared legacy executor and profile", () => {
  const key = { providerPreset: "kiro-api-key", adapter: "kiro_compatible", protocolProfile: "kiro", providerLabel: "Renamed" };
  expect(providerAccessKind(key)).toBe("api");
  expect(providerAccessLabel(key)).toBe("Renamed-api");
  expect(providerAccessKind({ ...key, providerPreset: "kiro" })).toBe("oauth");
});
it("keeps existing Accio, FreeBuff and Canvas product identities in channel labels", () => {
  expect(providerAccessLabel({ providerLabel: "accio-oauth", providerPreset: "accio" })).toBe("accio-oauth");
  expect(providerAccessLabel({ providerLabel: "freebuff-web", providerPreset: "freebuff" })).toBe("freebuff-web");
  expect(providerAccessLabel({ providerLabel: "gemini-canvas", providerPreset: "gemini-canvas" })).toBe("gemini-canvas");
  expect(providerAccessLabel({ providerLabel: "renamed-canvas", providerPreset: "gemini-canvas" })).toBe("renamed-canvas-web");
  expect(providerAccessLabel({ providerLabel: "gemini-canvas", providerPreset: "openai" })).toBe("gemini-canvas-api");
});
