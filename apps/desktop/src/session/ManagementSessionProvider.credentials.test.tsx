import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { readManagementSessionToken, writeManagementSessionToken } from "./storage";
import { createApi, Providers, SessionHarness } from "./managementSessionTestFixtures";

describe("ManagementSessionProvider", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("keeps secret grants in memory and out of browser storage", async () => {
    writeManagementSessionToken("stored-token");
    const api = createApi();
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /confirm secret/i }));

    expect(await screen.findByTestId("grant")).toHaveTextContent("short-lived-grant");
    expect(window.sessionStorage.length).toBe(1);
    expect(JSON.stringify(window.sessionStorage)).not.toContain("short-lived-grant");
    expect(window.localStorage.length).toBe(0);
  });

  it("clears both the in-memory grant and the session access flag", async () => {
    writeManagementSessionToken("stored-token");
    const api = createApi();
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /confirm secret/i }));
    expect(await screen.findByTestId("secret-access")).toHaveTextContent("true");

    await user.click(screen.getByRole("button", { name: /clear secret/i }));

    expect(screen.getByTestId("grant")).toHaveTextContent("none");
    expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
  });

  it("updates storage only after token rotation succeeds", async () => {
    writeManagementSessionToken("stored-token");
    let resolveRotate: ((value: { success: true }) => void) | undefined;
    const rotateSession = vi.fn(
      () =>
        new Promise<{ success: true }>((resolve) => {
          resolveRotate = resolve;
        }),
    );
    const api = createApi({ rotateSession });
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /rotate/i }));
    expect(readManagementSessionToken()).toBe("stored-token");

    await act(async () => resolveRotate?.({ success: true }));
    await waitFor(() => expect(readManagementSessionToken()).toBe("replacement-token"));
  });

  it("clears the browser session on explicit logout", async () => {
    writeManagementSessionToken("stored-token");
    const api = createApi();
    const user = userEvent.setup();

    render(
      <Providers api={api}>
        <SessionHarness />
      </Providers>,
    );
    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

    await user.click(screen.getByRole("button", { name: /log out/i }));

    await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("unauthenticated"));
    expect(readManagementSessionToken()).toBeNull();
    expect(api.logout).toHaveBeenCalledWith("stored-token");
  });
});
