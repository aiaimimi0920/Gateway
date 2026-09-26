import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ManagementSession } from "../api/contracts";
import { GatewayApiError } from "../api/errors";
import { createTauriHost } from "../platform/tauriHost";
import { authenticatedSession, createApi, deferred, Providers, SessionHarness } from "./managementSessionTestFixtures";

describe("ManagementSessionProvider", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("isolates persisted and in-memory sessions when the Tauri API origin changes", async () => {
    const api = createApi();
    const user = userEvent.setup();
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    await user.click(screen.getByRole("button", { name: /^log in$/i }));
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    await user.click(screen.getByRole("button", { name: /confirm secret/i }));
    expect(await screen.findByTestId("grant")).toHaveTextContent("short-lived-grant");
    expect(screen.getByTestId("management-token")).toHaveTextContent("profile-one-token");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("true");

    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );

    expect(screen.getByTestId("management-token")).toHaveTextContent("none");
    expect(screen.getByTestId("grant")).toHaveTextContent("none");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    expect(api.verifySession).toHaveBeenCalledTimes(1);
    expect(api.verifySession).toHaveBeenLastCalledWith("profile-one-token");

    view.rerender(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    expect(screen.getByTestId("management-token")).toHaveTextContent("profile-one-token");
    expect(api.verifySession).toHaveBeenCalledTimes(2);
    expect(api.verifySession).toHaveBeenLastCalledWith("profile-one-token");
  });

  it("ignores a pending login result from the previous Tauri API origin", async () => {
    const verification = deferred<ManagementSession>();
    const api = createApi({ verifySession: vi.fn(() => verification.promise) });
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    fireEvent.click(screen.getByRole("button", { name: /^log in$/i }));

    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));

    await act(async () => verification.resolve(authenticatedSession));

    expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated");
    expect(screen.getByTestId("management-token")).toHaveTextContent("none");
    expect(screen.getByTestId("grant")).toHaveTextContent("none");
  });

  it("ignores a pending authentication failure from the previous Tauri API origin", async () => {
    const verification = deferred<ManagementSession>();
    const api = createApi({ verifySession: vi.fn(() => verification.promise) });
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    fireEvent.click(screen.getByRole("button", { name: /^log in$/i }));
    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));

    await act(async () =>
      verification.reject(
        new GatewayApiError("Expired token", 401, "console_management_token_invalid"),
      ),
    );

    expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated");
    expect(screen.getByTestId("session-error")).toHaveTextContent("none");
  });

  it("ignores a pending secret grant from the previous Tauri API origin", async () => {
    const confirmation = deferred<{ grant: string; expiresAt: string }>();
    const api = createApi({ confirmSecretAccess: vi.fn(() => confirmation.promise) });
    const firstHost = createTauriHost("http://127.0.0.1:45123");
    const secondHost = createTauriHost("http://127.0.0.1:45124");
    const view = render(
      <Providers api={api} adapter={firstHost}>
        <SessionHarness />
      </Providers>,
    );

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    fireEvent.click(screen.getByRole("button", { name: /^log in$/i }));
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));
    fireEvent.click(screen.getByRole("button", { name: /confirm secret/i }));

    view.rerender(
      <Providers api={api} adapter={secondHost}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));

    await act(async () =>
      confirmation.resolve({
        grant: "stale-host-grant",
        expiresAt: "2099-01-01T00:00:00Z",
      }),
    );

    expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated");
    expect(screen.getByTestId("grant")).toHaveTextContent("none");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
  });
});
