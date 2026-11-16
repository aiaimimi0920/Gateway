import { expect, it } from "vitest";
import { isLumalabsMediaProvider, lumalabsProbeModels } from "./lumalabsProbeModels";

it("distinguishes Luma Web from Agents by preset, not mutable provider ID", () => {
  expect(isLumalabsMediaProvider({ id: "other", preset: "lumalabs" })).toBe(true);
  expect(isLumalabsMediaProvider({ id: "lumalabs", preset: "luma-agents" })).toBe(false);
});
it("only offers configured callable identities, not an invented online catalogue", () => {
  expect(lumalabsProbeModels({ supported_models: ["native"], model_map: { alias: "native", unavailable: "other" } },
    { supported_models: ["native"] })).toEqual(["native", "alias"]);
});
it("uses the standard 256 byte identity limit, not Udio's narrower binder", () => {
  expect(lumalabsProbeModels({ supported_models: ["x".repeat(256), "x".repeat(257), "native model", "bad*model"] }, {}))
    .toEqual(["x".repeat(256), "native model"]);
  expect(lumalabsProbeModels({ supported_models: ["alias"], model_map_targets: { alias: ["native", "x".repeat(257)] } },
    { supported_models: ["alias"] })).toEqual([]);
});
