import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { writeManagementSessionToken } from "./storage";
import { createApi, Providers, SessionHarness } from "./managementSessionTestFixtures";

describe("ManagementSessionProvider", () => {
  beforeEach(() => {
    window.localStorage.clear();
    window.sessionStorage.clear();
  });

  it("expires the in-memory secret grant and resets session access", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    vi.setSystemTime(new Date("2026-07-22T10:00:00.000Z"));
    try {
      writeManagementSessionToken("stored-token");
      const api = createApi({
        confirmSecretAccess: vi.fn().mockResolvedValue({
          grant: "short-lived-grant",
          expiresAt: "2026-07-22T10:00:01.000Z",
        }),
      });

      render(
        <Providers api={api}>
          <SessionHarness />
        </Providers>,
      );
      await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

      fireEvent.click(screen.getByRole("button", { name: /confirm secret/i }));
      await act(async () => Promise.resolve());
      expect(screen.getByTestId("grant")).toHaveTextContent("short-lived-grant");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("true");

      act(() => vi.advanceTimersByTime(1_001));

      expect(screen.getByTestId("grant")).toHaveTextContent("none");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
    } finally {
      vi.useRealTimers();
    }
  });

  it("reschedules secret grant expiry beyond the browser timer limit", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const start = new Date("2026-07-22T10:00:00.000Z");
    vi.setSystemTime(start);
    try {
      writeManagementSessionToken("stored-token");
      const expiresAt = new Date(start.getTime() + 2_147_483_647 + 1_000).toISOString();
      const api = createApi({
        confirmSecretAccess: vi.fn().mockResolvedValue({
          grant: "long-lived-grant",
          expiresAt,
        }),
      });

      render(
        <Providers api={api}>
          <SessionHarness />
        </Providers>,
      );
      await waitFor(() => expect(screen.getByTestId("phase")).toHaveTextContent("authenticated"));

      fireEvent.click(screen.getByRole("button", { name: /confirm secret/i }));
      await act(async () => Promise.resolve());
      expect(screen.getByTestId("grant")).toHaveTextContent("long-lived-grant");

      act(() => vi.advanceTimersByTime(2_147_483_647));
      expect(screen.getByTestId("grant")).toHaveTextContent("long-lived-grant");

      act(() => vi.advanceTimersByTime(1_001));
      expect(screen.getByTestId("grant")).toHaveTextContent("none");
      expect(screen.getByTestId("secret-access")).toHaveTextContent("false");
    } finally {
      vi.useRealTimers();
    }
  });
});
