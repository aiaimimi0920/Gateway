import { describe, expect, it } from "vitest";
import { consoleCredentialProbeResponseSchema } from "./credentials";
import { credentialTestAssessmentSchema } from "./credentialTests";

const base = { credentialId: "one", providerId: "pool", probePoint: "POST loopback", status: "passed", message: "OK", checkedAt: "2026-10-20" };

describe("model health recovery evidence", () => {
  it("preserves optional direct recovery and legacy responses", () => {
    expect(consoleCredentialProbeResponseSchema.parse({ result: base }).result).toEqual(base);
    const result = consoleCredentialProbeResponseSchema.parse({ result: { ...base, healthRecovery: "stale_health" } });
    expect(result.result).toHaveProperty("healthRecovery", "stale_health");
  });

  it("retains per-case recovery without claiming whole-plan recovery", () => {
    const assessment = credentialTestAssessmentSchema.parse({ policySource: "pool", mode: "manual", models: [], cases: [
      { caseId: "ok", name: "OK", model: "model", difficulty: 1, status: "passed", answer: "OK", expectedAnswer: null,
        correct: null, message: "OK", healthRecovery: "stale_configuration" },
    ] });
    expect(assessment.cases[0]).toHaveProperty("healthRecovery", "stale_configuration");
    expect(assessment).not.toHaveProperty("healthRecovery");
  });
});
