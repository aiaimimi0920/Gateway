import { useCallback, useMemo } from "react";
import type { ConsoleRouteDocument, CredentialTestPolicy, CredentialTestScope } from "../../api/contracts";
import { parseRouteDocument } from "./routeDocument";
import { testPolicyError, updateTestPolicies } from "./credentialTestPolicyDocument";
import { pushAppToast } from "../../components/AppToast";
import { updateTestPlans, type TestPlanChange } from "./credentialTestPlansDocument";

export function useTestPolicyEditor(editorText: string, replaceEditorDocument: (document: ConsoleRouteDocument, sync?: boolean) => void,
  setError: (error: string | null) => void, t: (zh: string, en: string) => string) {
  const testPolicyDocument = useMemo(() => { try { return parseRouteDocument(editorText); } catch { return null; } }, [editorText]);
  const applyTestPolicy = useCallback((scope: CredentialTestScope | CredentialTestScope[], policy: CredentialTestPolicy | null) => {
    try {
      if (policy) { const error = testPolicyError(policy); if (error) throw new Error(error); }
      replaceEditorDocument(updateTestPolicies(parseRouteDocument(editorText), Array.isArray(scope) ? scope : [scope], policy), true);
      setError(null);
      pushAppToast("info", t("策略已更新", "Policy updated"));
      return true;
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return false; }
  }, [editorText, replaceEditorDocument, setError, t]);
  const applyTestPlanChanges = useCallback((changes: TestPlanChange[]) => {
    try {
      replaceEditorDocument(updateTestPlans(parseRouteDocument(editorText), changes), true);
      setError(null);
      pushAppToast("info", t("测试计划已更新", "Test plans updated"));
      return true;
    } catch (cause) { setError(cause instanceof Error ? cause.message : String(cause)); return false; }
  }, [editorText, replaceEditorDocument, setError, t]);
  return { testPolicyDocument, applyTestPolicy, applyTestPlanChanges };
}
