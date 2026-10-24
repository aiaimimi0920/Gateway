import { expect, it } from "vitest";
import { groupCardModels } from "./modelDisplayCatalog";

it.each([
  { id: "moonshot", label: "Kimi", models: ["kimi-new-version", "moonshotai/kimi-new-version:free",
    "moonshot/kimi-new-version:batch", "Kimi/Kimi-New-Version[free]"] },
  { id: "xiaomi", label: "Xiaomi", models: ["mimo-new-version", "xiaomi/mimo-new-version:free",
    "Xiaomi/MiMo-New-Version[free]"] },
  { id: "google", label: "Google", models: ["gemini-new-version", "google/gemini-new-version-preview",
    "Google/Gemini-New-Version:batch"] },
  { id: "agnes", label: "Agnes", models: ["agnes-new-version", "agnes/agnes-new-version:free",
    "Agnes/Agnes-New-Version[free]"] },
])("groups the $label family without adding or rewriting capabilities", (group) => {
  expect(groupCardModels(group.models)).toEqual([group]);
});

it("merges listed and unlisted Kimi models into the same stable company group", () => {
  expect(groupCardModels(["kimi-new-version", "moonshotai/kimi-k2.5"])).toEqual([
    { id: "moonshot", label: "Kimi", models: ["moonshotai/kimi-k2.5", "kimi-new-version"] },
  ]);
});

it("keeps conflicting namespaces, unrelated names and malformed IDs unclassified", () => {
  const models = ["openai/kimi-new-version", "custom/mimo-new-version", "xiaomi/gemini-new-version",
    "google/agnes-new-version", "agnes/mimo-new-version", "proxy/google/gemini-new-version",
    "not-kimi-new-version", "gemini-", "agnes-", "mimosa"];
  expect(groupCardModels(models)).toEqual([{ id: "other", label: "other", models }]);
  expect(groupCardModels([])).toEqual([]);
});
