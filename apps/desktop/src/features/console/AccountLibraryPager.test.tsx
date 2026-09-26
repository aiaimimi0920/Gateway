import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  AccountLibraryPager,
  type AccountsLedgerPilotAccount,
} from "./ProviderAccountCard";

const t = (zh: string, _en: string) => zh;
let resizeCallback: ResizeObserverCallback | null = null;
const disconnect = vi.fn();

function account(index: number): AccountsLedgerPilotAccount {
  return {
    accountId: `account-${index}`,
    providerId: "provider",
    displayName: `Account ${index}`,
    mode: "credential",
    enabled: true,
    logicalLabels: [],
    capacityLabel: "1 / 1",
    statusLabel: "正常",
    dispatchEnabled: true,
    dispatchEditable: true,
    previewOnly: false,
    usageWindowBadges: [],
    recentUseLabel: "从未使用",
    verificationStatus: "not-tested",
    verificationFamilies: [],
    verificationCheckedAt: null,
    verificationEvidenceRef: null,
    verificationNote: "",
  };
}

beforeEach(() => {
  resizeCallback = null;
  disconnect.mockClear();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 1_200,
    height: 0,
    top: 0,
    right: 1_200,
    bottom: 0,
    left: 0,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        resizeCallback = callback;
      }
      observe() {}
      unobserve() {}
      disconnect() {
        disconnect();
      }
    },
  );
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("AccountLibraryPager", () => {
  it("resets the page when the library changes and responds to container width", async () => {
    const user = userEvent.setup();
    const accounts = Array.from({ length: 10 }, (_, index) => account(index + 1));
    const renderAccount = (entry: AccountsLedgerPilotAccount) => (
      <article data-account-card={entry.accountId} key={entry.accountId}>
        {entry.displayName}
      </article>
    );
    const view = render(
      <AccountLibraryPager
        t={t}
        libraryKey="alpha"
        accounts={accounts}
        renderAccount={renderAccount}
      />,
    );
    const grid = document.querySelector("[data-account-library-page]");

    expect(grid).toHaveAttribute("data-account-library-page-size", "8");
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(grid).toHaveAttribute("data-account-library-page", "2");
    expect(screen.getByText("Account 9")).toBeInTheDocument();

    view.rerender(
      <AccountLibraryPager
        t={t}
        libraryKey="beta"
        accounts={accounts}
        renderAccount={renderAccount}
      />,
    );
    expect(grid).toHaveAttribute("data-account-library-page", "1");
    expect(screen.getByText("Account 1")).toBeInTheDocument();

    act(() => {
      resizeCallback?.(
        [{ contentRect: { width: 600 } } as unknown as ResizeObserverEntry],
        {} as ResizeObserver,
      );
    });
    expect(grid).toHaveAttribute("data-account-library-page-size", "4");

    view.unmount();
    expect(disconnect).toHaveBeenCalledOnce();
  });

  it("clamps direct page entry to the valid range", async () => {
    const user = userEvent.setup();
    const accounts = Array.from({ length: 18 }, (_, index) => account(index + 1));
    render(
      <AccountLibraryPager
        t={t}
        libraryKey="alpha"
        accounts={accounts}
        renderAccount={(entry) => <span key={entry.accountId}>{entry.displayName}</span>}
      />,
    );
    const grid = document.querySelector("[data-account-library-page]");
    const input = screen.getByRole("spinbutton", { name: "跳转页数" });

    await user.clear(input);
    await user.type(input, "99{Enter}");
    expect(grid).toHaveAttribute("data-account-library-page", "3");

    await user.clear(input);
    await user.type(input, "0{Enter}");
    expect(grid).toHaveAttribute("data-account-library-page", "1");
  });
});
