import { act, renderHook } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import type { ConsoleGeminiAuthFamily, ConsoleGeminiAuthSession } from "../../api/contracts";
import { GatewayApiError } from "../../api/errors";
import { pushAppToast } from "../../components/AppToast";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useConsoleActionIdentity } from "./useConsoleActionIdentity";
import { useConsoleRouteDraft } from "./useConsoleRouteDraft";
import { useGeminiCredentialImport } from "./useGeminiCredentialImport";

vi.mock("../../components/AppToast", () => ({ pushAppToast: vi.fn() }));
beforeEach(() => vi.clearAllMocks());
const t = (_zh: string, en: string) => en;
const invalidate = () => {};
const groups = () => [];

async function setup(family: ConsoleGeminiAuthFamily = "gemini-canvas") {
  const api = createConsoleApi();
  const baseline = await api.getRouteConfig("token");
  const document = { providers: [{ id: family, credentials: [] }], model_routes: [], aliases: {}, account_groups: [] };
  const routeConfig = { routeConfig: { ...baseline.routeConfig, document, secrets: [] } };
  const session: ConsoleGeminiAuthSession = {
    id: "session", providerId: family, targetFamily: family, status: "succeeded",
    message: "captured", createdAt: "2026-09-09T00:00:00Z", updatedAt: "2026-09-09T00:00:00Z",
    generatedDrafts: [{ providerId: family, credential: { id: "new-account" }, secretEdits: [] }],
  };
  const error = vi.fn();
  const refresh = vi.fn(async () => {});
  const setRouteConfig = vi.fn();
  const onRouteConfigHydrated = vi.fn();
  const setSecretDialogOpen = vi.fn();
  const hook = renderHook(({ token }) => {
    const identity = useConsoleActionIdentity({ api, managementToken: token, session: { secretGrant: null } });
    const draft = useConsoleRouteDraft({
      routeConfig,
      hasSecretAccess: false,
      t,
      invalidateCredentialProbes: invalidate,
      accountGroupDraftRowsFromDocument: groups,
      onRouteConfigHydrated,
      setError: error,
      setSecretDialogOpen,
    });
    return useGeminiCredentialImport({ ...draft, ...identity, managementToken: token,
      routeConfig, editorText: JSON.stringify(document), setRouteConfig, setError: error,
      refresh, t, handleSecretAccessRequiredError: (cause, current) => {
        if (!(cause instanceof GatewayApiError) || cause.code !== "console_secret_access_required") return "not-required";
        return current() ? "recovered" : "stale";
      } });
  }, { initialProps: { token: "token" } });
  return { api, session, error, refresh, setRouteConfig, ...hook };
}

it.each(["unmount", "token"] as const)("suppresses stale recovery fallback after %s", async (change) => {
  const h = await setup();
  let reject!: (error: Error) => void;
  vi.mocked(h.api.commitRouteConfig).mockReturnValueOnce(new Promise<never>((_yes, no) => { reject = no; }));
  act(() => h.result.current.applyGeminiManualAddSessionResult(h.session));
  expect(h.api.commitRouteConfig).toHaveBeenCalledOnce();
  if (change === "unmount") h.unmount();
  else h.rerender({ token: "new-token" });
  await act(async () => { reject(new GatewayApiError("grant expired", 403, "console_secret_access_required")); });
  expect(pushAppToast).not.toHaveBeenCalled();
  expect(h.setRouteConfig).not.toHaveBeenCalled();
});

it("suppresses immediate draft-only fallback when unmounted before its microtask", async () => {
  const h = await setup("gemini-business");
  await act(async () => {
    h.result.current.applyGeminiManualAddSessionResult(h.session);
    h.unmount();
  });
  expect(h.api.commitRouteConfig).not.toHaveBeenCalled();
  expect(pushAppToast).not.toHaveBeenCalled();
});

it("applies a completed session once and leaves failed persistence for ordinary save", async () => {
  const h = await setup();
  vi.mocked(h.api.commitRouteConfig).mockRejectedValue(new Error("commit unavailable"));
  await act(async () => h.result.current.applyGeminiManualAddSessionResult(h.session));
  await act(async () => h.result.current.applyGeminiManualAddSessionResult(h.session));
  expect(h.api.commitRouteConfig).toHaveBeenCalledOnce();
  expect(h.error).toHaveBeenLastCalledWith("commit unavailable");
  expect(pushAppToast).toHaveBeenCalledOnce();
  expect(pushAppToast).toHaveBeenCalledWith("info", expect.stringContaining("Save the route config"));
});
