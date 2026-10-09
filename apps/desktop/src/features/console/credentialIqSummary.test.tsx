import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import type { ConsoleCredentialProbeResult } from "../../api/contracts";
import { testIqRows } from "./credentialIqSummary";
import { CredentialTestModelSummary } from "./CredentialTestModelSummary";
import { credentialTestAssessmentSchema } from "../../api/schemas/credentialTests";
import { appendIqCases, iqExtraCases } from "./credentialIqBank";
import { defaultTestPolicy, testPolicyError } from "./credentialTestPolicyDocument";

function result(): ConsoleCredentialProbeResult {
  return { providerId: "p", credentialId: "one", probePoint: "matrix", status: "passed", message: "done", checkedAt: "2026-10-06T00:00:00Z",
    assessment: { planId: "plan", policySource: "plan:plan", mode: "manual", models: [{ model: "m", callable: true, completedCount: 2,
      gradedCount: 2, correctCount: 1, score: 50, capabilityLevel: "below-standard", measurement: { testSetId: "a".repeat(64), elapsedMs: 1500,
        quota: { status: "account-observed", unit: "credits", consumed: 3, before: 20, after: 17, source: "provider-balance" } } }],
      cases: [{ caseId: "easy", name: "easy", model: "m", difficulty: 1, status: "passed", answer: "1", expectedAnswer: "1", correct: true, message: "done", elapsedMs: 500 },
        { caseId: "hard", name: "hard", model: "m", difficulty: 3, status: "passed", answer: "0", expectedAnswer: "2", correct: false, message: "done", elapsedMs: 1000 }] } };
}

it("uses difficulty weights and exposes only explicitly observational ratios", () => {
  expect(testIqRows([result()])[0]).toMatchObject({ iq: 25, observed: 3, quotaPerIq: 0.12, iqPerQuota: 25 / 3, elapsedMs: 1500 });
  render(<CredentialTestModelSummary results={[result()]} t={(zh) => zh} />);
  for (const name of ["测试 IQ", "额度消耗（账户观测）", "完成用时", "额度 / IQ · 越低越好", "IQ / 额度 · 越高越好"])
    expect(screen.getByText(name)).toBeInTheDocument();
  expect(screen.getByText(/不是请求级账单/)).toBeInTheDocument();
  expect(screen.getByText("1.5 s")).toBeInTheDocument();
});

it("does not turn old, incomplete or ungraded evidence into IQ or free usage", () => {
  const old = result(); delete old.assessment!.models[0].measurement;
  expect(credentialTestAssessmentSchema.parse(old.assessment).models[0].measurement).toBeUndefined();
  expect(testIqRows([old])[0]).toMatchObject({ iq: null, elapsedMs: null, observed: null, quotaPerIq: null });
  for (const status of ["not-run", "failed", "unsupported"]) {
    const value = result(); value.assessment!.cases[1].status = status;
    expect(testIqRows([value])[0].iq).toBeNull();
  }
  for (const status of ["unavailable", "reset-or-replenished", "no-visible-change"]) {
    const value = result(); value.assessment!.models[0].measurement!.quota.status = status;
    expect(testIqRows([value])[0]).toMatchObject({ observed: null, quotaPerIq: null, iqPerQuota: null });
  }
});

it("handles zero intelligence without infinity and separates account/provider evidence", () => {
  const zero = result(); zero.assessment!.cases.forEach((item) => { item.correct = false; });
  expect(testIqRows([zero])[0]).toMatchObject({ iq: 0, quotaPerIq: null, iqPerQuota: 0 });
  const other = result(); other.credentialId = "two";
  const newer = result(); newer.checkedAt = "2026-10-06T01:00:00Z";
  expect(testIqRows([result(), other, newer])).toHaveLength(2);
  expect(testIqRows([result(), newer])[0].latest).toBe(newer.checkedAt);
});

it("extends the original bank to twelve without silently enabling extra paid calls", () => {
  const policy = defaultTestPolicy();
  expect(policy.cases).toHaveLength(12);
  expect(policy.cases.filter((item) => item.enabled)).toHaveLength(1);
  expect(testPolicyError(policy)).toBeNull();
  expect(appendIqCases(policy.cases)).toEqual(policy.cases);
  expect(appendIqCases(policy.cases.slice(0, 3))).toEqual(policy.cases);
  expect(new Set(iqExtraCases.map((item) => item.id)).size).toBe(9);
  const count = Array.from({ length: 120 }, (_, i) => i + 1).filter((n) => (n % 4 === 0) !== (n % 6 === 0)).length;
  const modular = Array.from({ length: 315 }, (_, i) => i + 1).find((n) => n % 5 === 2 && n % 7 === 3 && n % 9 === 4);
  expect(String(count)).toBe(iqExtraCases.find((item) => item.id === "iq-count-v1")!.expectedAnswer);
  expect(String(modular)).toBe(iqExtraCases.find((item) => item.id === "iq-modular-v1")!.expectedAnswer);
});
