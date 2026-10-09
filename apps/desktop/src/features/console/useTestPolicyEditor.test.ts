import { act, renderHook } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useTestPolicyEditor } from "./useTestPolicyEditor";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";

it("commits a multi-target selection once and rejects an invalid batch without partial writes", () => {
  const document = { providers: [{ id: "p", supported_models: ["a"], credentials: [{ id: "one", api_key: "preserved" }],
    credential_identity_categories: [{ id: "free", label: "Free" }, { id: "plus", label: "Plus" }] }],
    model_routes: [], aliases: {}, account_groups: [] };
  const replace = vi.fn(); const error = vi.fn();
  const hook = renderHook(() => useTestPolicyEditor(JSON.stringify(document), replace, error, (zh) => zh));
  act(() => { expect(hook.result.current.applyTestPolicy([
    { kind: "subpool", providerId: "p", id: "free" }, { kind: "subpool", providerId: "p", id: "plus" },
  ], defaultTestPolicy())).toBe(true); });
  expect(replace).toHaveBeenCalledTimes(1);
  expect(replace.mock.calls[0][0].providers[0]).toMatchObject({
    subpool_test_policies: { free: defaultTestPolicy(), plus: defaultTestPolicy() },
    credentials: [{ id: "one", api_key: "preserved" }],
  });
  act(() => { expect(hook.result.current.applyTestPolicy([
    { kind: "subpool", providerId: "p", id: "free" }, { kind: "account", providerId: "missing", id: "x" },
  ], defaultTestPolicy())).toBe(false); });
  expect(replace).toHaveBeenCalledTimes(1);
});
