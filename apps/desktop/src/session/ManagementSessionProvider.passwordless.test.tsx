import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createApi, Providers, SessionHarness } from "./managementSessionTestFixtures";
import { bootstrapStatusSchema } from "../api/schemas/core";

beforeEach(() => { localStorage.clear(); sessionStorage.clear(); vi.stubGlobal("__GATEWAY_MANAGEMENT_LOGIN_REQUIRED__", true); });
afterEach(() => vi.unstubAllGlobals());

it("automatically verifies the public session marker even in a login-required build", async () => {
  const status = { needsBootstrap: false, managementConfigured: true, environmentOverride: false, passwordlessToken: "passwordless:revision-1" };
  expect(bootstrapStatusSchema.parse(status)).toEqual(status);
  const api = createApi({ getBootstrapStatus: vi.fn().mockResolvedValue(status) });
  render(<Providers api={api}><SessionHarness /></Providers>);
  await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
  expect(api.verifySession).toHaveBeenCalledWith(status.passwordlessToken);
  await waitFor(() => expect(api.confirmSecretAccess).toHaveBeenCalledWith(status.passwordlessToken, status.passwordlessToken));
});

it("does not enter the console if a stale public marker is rejected", async () => {
  const api = createApi({
    getBootstrapStatus: vi.fn().mockResolvedValue({ needsBootstrap: false, managementConfigured: true, environmentOverride: false, passwordlessToken: "passwordless:old" }),
    verifySession: vi.fn().mockRejectedValue(new Error("Session changed")),
  });
  render(<Providers api={api}><SessionHarness /></Providers>);
  await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("error"));
  expect(api.confirmSecretAccess).not.toHaveBeenCalled();
});
