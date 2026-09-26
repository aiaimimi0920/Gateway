import { act, renderHook } from "@testing-library/react";
import { expect, it } from "vitest";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { useConsoleActionIdentity } from "./useConsoleActionIdentity";

function setup() {
  const props: Parameters<typeof useConsoleActionIdentity>[0] = {
    api: createConsoleApi(), managementToken: "token-a",
    session: { secretGrant: { grant: "grant-a", expiresAt: "2099-01-01T00:00:00Z" } },
  };
  return { props, ...renderHook(value => useConsoleActionIdentity(value), { initialProps: props }) };
}

it.each(["unmount", "token", "api"] as const)("invalidates requests permanently after %s transition", (change) => {
  const h = setup();
  const identity = h.result.current;
  const request = identity.beginConsoleActionRequest("token-a");
  expect(identity.isConsoleActionRequestCurrent(request)).toBe(true);
  if (change === "unmount") h.unmount();
  else {
    h.rerender({ ...h.props, ...(change === "api" ? { api: createConsoleApi() } : { managementToken: "token-b" }) });
    h.rerender(h.props);
  }
  expect(identity.isConsoleActionRequestCurrent(request)).toBe(false);
  expect(identity.isConsoleActionRecoveryCurrent(request)).toBe(false);
  expect(identity.consoleActionGenerationRef.current).not.toBe(request.generation);
});

it("allows only the immediate revoked grant to recover", () => {
  const h = setup();
  const request = h.result.current.beginConsoleActionRequest("token-a");
  h.rerender({ ...h.props, session: { secretGrant: null } });
  expect(h.result.current.isConsoleActionRequestCurrent(request)).toBe(false);
  expect(h.result.current.isConsoleActionRecoveryCurrent(request)).toBe(true);
  h.rerender(h.props);
  expect(h.result.current.isConsoleActionRecoveryCurrent(request)).toBe(false);
  h.rerender({ ...h.props, session: { secretGrant: null } });
  expect(h.result.current.isConsoleActionRecoveryCurrent(request)).toBe(false);
});

it("keeps only the latest action current", () => {
  const h = setup();
  const old = h.result.current.beginConsoleActionRequest("token-a");
  let current = old;
  act(() => { current = h.result.current.beginConsoleActionRequest("token-a"); });
  expect(h.result.current.isConsoleActionRequestCurrent(old)).toBe(false);
  expect(h.result.current.isConsoleActionRequestCurrent(current)).toBe(true);
});

it("clears obsolete save busy state when the host identity changes", () => {
  const h = setup();
  act(() => h.result.current.setActionBusy("save"));
  expect(h.result.current.actionBusy).toBe("save");
  h.rerender({ ...h.props, managementToken: "token-b" });
  expect(h.result.current.actionBusy).toBeNull();
});

it("rejects recovery after direct grant replacement", () => {
  const h = setup();
  const request = h.result.current.beginConsoleActionRequest("token-a");
  h.rerender({ ...h.props, session: { secretGrant: { grant: "grant-b", expiresAt: "2099-01-01T00:00:00Z" } } });
  expect(h.result.current.isConsoleActionRequestCurrent(request)).toBe(false);
  expect(h.result.current.isConsoleActionRecoveryCurrent(request)).toBe(false);
});
