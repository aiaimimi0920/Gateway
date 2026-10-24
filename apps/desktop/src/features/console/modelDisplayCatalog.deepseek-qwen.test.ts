import { expect, it } from "vitest";
import { cardModelDisplayName, groupCardModels } from "./modelDisplayCatalog";

it("groups new DeepSeek and Qwen variants without changing capability IDs", () => {
  const deepseek = ["deepseek-v4.1-flash", "deepseek/deepseek-v4.1-flash:batch",
    "deepseek-ai/DeepSeek-V4.1-Flash[free]", "~deepseek/deepseek-flash-latest",
    "deepseek/deepseek-r1-distill-qwen-32b"];
  const qwen = ["qwen3.6-plus", "qwen/qwen3.6-35b-a3b:free", "Qwen/Qwen3-Coder-Next",
    "qwen-max", "qwen-plus-latest[free]"];
  expect(groupCardModels([...deepseek, ...qwen])).toEqual([
    { id: "deepseek", label: "DeepSeek", models: deepseek },
    { id: "qwen", label: "Qwen", models: qwen },
  ]);
  expect(cardModelDisplayName(deepseek[1])).toBe("deepseek-v4.1-flash:batch");
});

it("does not infer ownership from arbitrary substrings or conflicting namespaces", () => {
  const models = ["custom/deepseek-v4.1-flash", "openai/qwen3.6-plus", "qwen/deepseek-v4.1-flash",
    "deepseek/qwen3.6-plus", "org/deepseek/deepseek-v4.1-flash", "not-deepseek-v4",
    "qwentastic", "~custom/deepseek-flash-latest"];
  expect(groupCardModels(models)).toEqual([{ id: "other", label: "other", models }]);
  expect(groupCardModels([])).toEqual([]);
});
