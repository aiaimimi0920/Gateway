import { render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AuthBoundary } from "../features/auth/AuthBoundary";
import { createBrowserHost } from "../platform/browserHost";
import { createApi, Providers, SessionHarness } from "./managementSessionTestFixtures";
import { writeManagementSessionToken } from "./storage";

describe("devtest automatic management session", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });
  afterEach(() => vi.unstubAllGlobals());

  it("opens a fresh local console without asking for a password, after server verification", async () => {
    vi.stubGlobal("__GATEWAY_MANAGEMENT_LOGIN_REQUIRED__", false);
    const api = createApi();
    render(<Providers api={api} adapter={createBrowserHost({ origin: "http://127.0.0.1:4400" })}>
      <AuthBoundary><div>Console ready</div></AuthBoundary>
    </Providers>);
    expect(await screen.findByText("Console ready")).toBeInTheDocument();
    expect(api.verifySession).toHaveBeenCalledWith("11011101");
    expect(screen.queryByRole("heading", { name: /登录/ })).not.toBeInTheDocument();
  });

  it("does not reuse the previous isolated password in manual-test mode", async () => {
    vi.stubGlobal("__GATEWAY_MANAGEMENT_LOGIN_REQUIRED__", false);
    const origin = "http://127.0.0.1:4400";
    writeManagementSessionToken("obsolete-isolated-token", window.sessionStorage, origin);
    const api = createApi();
    render(<Providers api={api} adapter={createBrowserHost({ origin })}><SessionHarness /></Providers>);
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    expect(api.verifySession).toHaveBeenCalledWith("11011101");
  });

  it.each([true, undefined])("requires login for release or unspecified build policy (%s)", async (required) => {
    vi.stubGlobal("__GATEWAY_MANAGEMENT_LOGIN_REQUIRED__", required);
    const api = createApi();
    render(<Providers api={api}><SessionHarness /></Providers>);
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    expect(api.verifySession).not.toHaveBeenCalled();
  });

  it.each(["https://gateway.example", "http://127.0.0.1.example:4400"])("never sends the testing key to %s", async (origin) => {
    vi.stubGlobal("__GATEWAY_MANAGEMENT_LOGIN_REQUIRED__", false);
    const api = createApi();
    render(<Providers api={api} adapter={createBrowserHost({ origin })}><SessionHarness /></Providers>);
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    expect(api.verifySession).not.toHaveBeenCalled();
  });

  it("does not pretend authentication succeeded when the server rejects it", async () => {
    vi.stubGlobal("__GATEWAY_MANAGEMENT_LOGIN_REQUIRED__", false);
    const api = createApi({ verifySession: vi.fn().mockRejectedValue(new Error("server refused session")) });
    render(<Providers api={api} adapter={createBrowserHost({ origin: "http://localhost:4400" })}><SessionHarness /></Providers>);
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("error"));
    expect(screen.getByTestId("session-error")).toHaveTextContent("server refused session");
  });
});
