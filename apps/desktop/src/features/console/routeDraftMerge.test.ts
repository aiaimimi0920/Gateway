import { expect, it } from "vitest";
import { mergeRouteValue } from "./routeDraftMerge";

it("merges distinct provider fields and retains remote credential refresh metadata", () => {
  const base = { providers: [{ id: "one", label: "Before", credentials: [{ id: "a", expires_at: 10 }] }] };
  const draft = structuredClone(base); draft.providers[0].label = "After";
  const latest = structuredClone(base); latest.providers[0].credentials[0].expires_at = 20;
  expect(mergeRouteValue(base, draft, latest)).toEqual({ providers: [{ id: "one", label: "After", credentials: [{ id: "a", expires_at: 20 }] }] });
  expect(base.providers[0].label).toBe("Before");
});

it("merges by identity after reorder and preserves independently added providers", () => {
  const base = { providers: [{ id: "a", label: "A" }, { id: "b", label: "B" }] };
  const draft = { providers: [{ id: "a", label: "Changed" }, base.providers[1]] };
  const latest = { providers: [base.providers[1], base.providers[0], { id: "c", label: "C" }] };
  expect(mergeRouteValue(base, draft, latest)).toEqual({ providers: [base.providers[1], draft.providers[0], { id: "c", label: "C" }] });
});

it("keeps a local priority reorder while preserving remote field edits", () => {
  const base = { providers: [{ id: "a", label: "A" }, { id: "b", label: "B" }] };
  const draft = { providers: [...base.providers].reverse() };
  const latest = { providers: [{ id: "a", label: "New A" }, base.providers[1]] };
  expect(mergeRouteValue(base, draft, latest)).toEqual({ providers: [base.providers[1], latest.providers[0]] });
});

it.each([
  [{ providers: [{ id: "a", label: "A" }] }, { providers: [{ id: "a", label: "Local" }] }, { providers: [{ id: "a", label: "Remote" }] }],
  [{ providers: [{ id: "a", label: "A" }] }, { providers: [] }, { providers: [{ id: "a", label: "Remote" }] }],
  [{ model_routes: ["a", "b"] }, { model_routes: ["b", "a"] }, { model_routes: ["a", "c"] }],
])("refuses overlapping edits instead of overwriting them", (base, draft, latest) => {
  expect(() => mergeRouteValue(base, draft, latest)).toThrow(/冲突/);
});

it("treats prototype-looking alias names as data and preserves field deletion", () => {
  const base = JSON.parse('{"aliases":{"__proto__":"a","old":"x"}}');
  const draft = JSON.parse('{"aliases":{"__proto__":"b"}}');
  const latest = JSON.parse('{"aliases":{"__proto__":"a","old":"x","constructor":"c"}}');
  const result = mergeRouteValue(base, draft, latest) as { aliases: Record<string, string> };
  expect(Object.keys(result.aliases)).toEqual(["__proto__", "constructor"]);
  expect(Object.getPrototypeOf(result.aliases)).toBe(Object.prototype);
  expect(result.aliases.__proto__).toBe("b");
});
