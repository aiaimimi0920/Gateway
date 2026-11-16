import { expect, it } from "vitest";
import { chataibotProbeModels } from "./chataibotProbeModels";

it("does not invent entitlement and resolves configured aliases under the exact account allowlist", () => {
  expect(chataibotProbeModels({}, {}, false)).toEqual([]);
  const provider = { supported_models: ["ideogram", "google-nano-banana-2"], model_map: { alias: "google-nano-banana-2", foreign: "gpt-image-1.5" } };
  expect(chataibotProbeModels(provider, { supported_models: ["google-nano-banana-2"] }, false)).toEqual(["google-nano-banana-2", "alias"]);
  expect(chataibotProbeModels(provider, {}, true)).toEqual(["google-nano-banana-2", "alias"]);
});
it("does not choose only the convenient member of a random eligible multi-target mapping", () => {
  const provider = { model_map: { alias: "google-nano-banana-2" }, model_map_targets: { alias: ["ideogram", "google-nano-banana-2"] } };
  expect(chataibotProbeModels(provider, {}, true)).toEqual([]);
  expect(chataibotProbeModels(provider, { supported_models: ["google-nano-banana-2"] }, true)).toEqual(["google-nano-banana-2", "alias"]);
  expect(chataibotProbeModels(provider, { supported_models: ["alias"] }, true)).toEqual([]);
  expect(chataibotProbeModels({ supported_models: ["ideogram"], model_map_targets: { ideogram: [] } }, {}, false)).toEqual([]);
});
