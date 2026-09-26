import { expect, it } from "vitest";
import { readPilotIdentityCategories } from "./pilotPoolPolicy";

it("keeps the first category policy after ID normalization without mutating input", () => {
  const categories = [
    Object.freeze({ id: " __proto__ ", label: "First", pool_target_size: 7 }),
    Object.freeze({ id: "__proto__", label: "Duplicate", pool_target_size: 99 }),
    "Free Tier",
    Object.freeze({ id: "free-tier", label: "Duplicate free", auto_refill_enabled: true }),
    Object.freeze({ id: "constructor", label: "Last", auto_prune_enabled: true }),
  ];
  Object.freeze(categories);
  const provider = Object.freeze({ credential_identity_categories: categories });

  expect(readPilotIdentityCategories(provider, "custom", "Custom")).toEqual([
    { id: "__proto__", label: "First", poolTargetSize: 7,
      autoRefillEnabled: false, autoPruneEnabled: false },
    { id: "free-tier", label: "Free Tier", poolTargetSize: 30,
      autoRefillEnabled: false, autoPruneEnabled: false },
    { id: "constructor", label: "Last", poolTargetSize: 30,
      autoRefillEnabled: false, autoPruneEnabled: true },
  ]);
  expect(categories).toHaveLength(5);
  expect(categories[0]).toHaveProperty("id", " __proto__ ");
});
