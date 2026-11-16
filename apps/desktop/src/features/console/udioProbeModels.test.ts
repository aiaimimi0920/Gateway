import { expect, it } from "vitest";
import { isUdioMediaProvider, udioProbeModels } from "./udioProbeModels";

it("uses the preset, never the provider ID, to recognize Udio", () => {
  expect(isUdioMediaProvider({ id: "different", preset: "udio" })).toBe(true);
  expect(isUdioMediaProvider({ id: "udio", preset: "suno" })).toBe(false);
});
it("preserves configured opaque models and aliases without inventing a current catalogue", () => {
  expect(udioProbeModels({ supported_models: ["native"], model_map: { alias: "native", unavailable: "other" } },
    { supported_models: ["native"] })).toEqual(["native", "alias"]);
});
it("requires every allowed mapping to fit the actual 128 byte native binder", () => {
  const provider = { supported_models: ["alias"], model_map_targets: { alias: ["native", "x".repeat(129)] } };
  expect(udioProbeModels(provider, { supported_models: ["alias"] })).toEqual([]);
  expect(udioProbeModels({ supported_models: ["x".repeat(128), "native model", "m\u0085odel"] }, {})).toEqual(["x".repeat(128)]);
});
