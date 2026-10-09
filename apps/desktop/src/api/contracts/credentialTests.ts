// Additive test-plan and evidence contracts shared by auto/manual and all scopes.
export type CredentialTestCase = {
  id: string; name: string; prompt: string; expectedAnswer?: string | null; difficulty: number; enabled: boolean;
};
export type CredentialTestPolicy = {
  automaticEnabled: boolean; intervalMinutes: number; modelSelection: "all" | "selected";
  models: string[]; cases: CredentialTestCase[];
};
export type CredentialTestRequestScope = (
  { kind: "pool" } | { kind: "subpool"; id: string } | { kind: "account"; id: string }
);
export type CredentialTestScope = { providerId: string } & CredentialTestRequestScope;
export type CredentialTestPlan = {
  id: string; name: string; scopes: CredentialTestRequestScope[]; policy: CredentialTestPolicy;
};
export type CredentialTestResultQuery = { scope?: CredentialTestRequestScope; planId?: string; includePlans?: boolean };
export type TestModelMeasurement = {
  testSetId: string; elapsedMs: number | null;
  quota: { status: string; unit: string | null; consumed: number | null; before: number | null; after: number | null; source: string | null };
};
export type CredentialTestAssessment = {
  planId?: string;
  policySource: string; mode: string;
  models: { model: string; callable: boolean; completedCount: number; gradedCount: number; correctCount: number;
    score: number | null; capabilityLevel: string; measurement?: TestModelMeasurement }[];
  cases: { caseId: string; name: string; model: string; difficulty: number; status: string;
    answer: string | null; expectedAnswer: string | null; correct: boolean | null; message: string; elapsedMs?: number }[];
};
