import { expect, it } from "vitest";
import type { ConsoleCredentialProbeResult } from "../../api/contracts";
import { summarizeTestModels } from "./credentialTestSummary";
import { testPlanResultRows } from "./useTestPlanResults";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";

function result(planId: string, statuses: [string, boolean | null][]): ConsoleCredentialProbeResult {
  return { providerId: "p", credentialId: "one", probePoint: "matrix", status: "passed", message: "round", checkedAt: "2026-10-05T00:00:00Z",
    assessment: { planId, policySource: `plan:${planId}`, mode: "manual", models: [{ model: "a", callable: true, completedCount: 1,
      gradedCount: 1, correctCount: 1, score: 100, capabilityLevel: "advanced" }],
    cases: statuses.map(([status, correct], index) => ({ caseId: String(index), name: "case", model: "a", difficulty: 3,
      status, correct, answer: "preview", expectedAnswer: "OK", message: "complete" })) } };
}

it("keeps callable wrong answers separate from failures, unsupported and not-run", () => {
  const [row] = summarizeTestModels([result("first", [["passed", false], ["failed", null], ["unsupported", null], ["not-run", null]]),
    result("second", [["passed", true], ["passed", null]])]);
  expect(row).toMatchObject({ attempted: 4, passed: 3, failed: 1, graded: 2, correct: 1,
    connectivity: 75, errorRate: 25, score: 50, answerErrorRate: 50, unsupported: 1, notRun: 1, level: "incomplete" });
  expect(row.plans.size).toBe(2);
});

it("does not infer intelligence or failures from ungraded or unexecuted evidence", () => {
  expect(summarizeTestModels([result("first", [["passed", null]])])[0]).toMatchObject({ score: null, answerErrorRate: null, level: "unrated" });
  expect(summarizeTestModels([result("first", [["unsupported", null], ["not-run", null]])])[0]).toMatchObject({ connectivity: null, errorRate: null, attempted: 0 });
  expect(summarizeTestModels([result("first", [["passed", false]])])[0]).toMatchObject({ connectivity: 100, errorRate: 0, level: "below-standard" });
});

it("deduplicates by provider/account/plan and uses only the latest round", () => {
  const old = result("first", [["passed", false]]);
  const fresh = { ...result("first", [["passed", true]]), checkedAt: "2026-10-05T01:00:00Z" };
  expect(summarizeTestModels([fresh, old, fresh])[0]).toMatchObject({ attempted: 1, score: 100, level: "advanced" });
});

it("keeps legacy account result expansion bound to the exact account", () => {
  const one = result("first", [["passed", true]]);
  delete one.assessment!.planId; one.assessment!.policySource = "account";
  const two = { ...one, credentialId: "two" };
  const row = { key: "legacy-one", id: "legacy-one", name: "One", providerId: "p", legacy: true, policy: defaultTestPolicy(),
    scopes: [{ kind: "account" as const, providerId: "p", id: "one" }] };
  expect(testPlanResultRows(row, [one, two])).toEqual([one]);
});
