import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AccountLibraryPager } from "./AccountLibraryPager";
import { pilotAccount } from "./AccountsLedgerWorkspace.fixtures";
import type { AccountsLedgerPilotAccount } from "./accountCardTypes";

const t = (zh: string, _en: string) => zh;
const accounts = Array.from({ length: 18 }, (_, index) => pilotAccount({ accountId: `acct-${index}` }));
const renderAccount = (account: AccountsLedgerPilotAccount) => (
  <article key={account.accountId} data-account-card={account.accountId}>{account.displayName}</article>
);
let width = 1_200;
let resizeCallback: ResizeObserverCallback | null = null;
const disconnect = vi.fn();

const pager = () => screen.getByRole("region", { name: "账号库滚动区域" });
const rows = () => pager().querySelector("[data-account-library-rows]");
const notifyResize = () => act(() => resizeCallback?.([], {} as ResizeObserver));

beforeEach(() => {
  width = 1_200;
  resizeCallback = null;
  disconnect.mockClear();
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(() => ({
    width, height: 0, top: 0, right: width, bottom: 0, left: 0, x: 0, y: 0, toJSON: () => ({}),
  }));
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

describe("AccountLibraryPager stable viewport", () => {
  it("reserves one row for an entirely empty library", () => {
    render(<AccountLibraryPager t={t} libraryKey="empty" accounts={[]} maxAccountCount={0} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    expect(pager()).toHaveStyle({ "--nt-account-library-visible-rows": "1" });
    expect(screen.getByText("当前账号库为空")).toBeInTheDocument();
    expect(pager()).toHaveAttribute("tabindex", "0");
  });

  it("keeps one row when switching from a populated to an empty tab", () => {
    const view = render(<AccountLibraryPager t={t} libraryKey="free" accounts={accounts.slice(0, 2)}
      maxAccountCount={2} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    view.rerender(<AccountLibraryPager t={t} libraryKey="plus" accounts={[]}
      maxAccountCount={2} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    expect(disconnect).not.toHaveBeenCalled();
  });

  it("caps the viewport at two rows but keeps every account in the scroll region", () => {
    const view = render(<AccountLibraryPager t={t} libraryKey="many" accounts={accounts}
      maxAccountCount={18} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
    expect(pager().querySelectorAll("[data-account-card]")).toHaveLength(18);
    expect(screen.queryByRole("button", { name: "下一页" })).not.toBeInTheDocument();
    view.rerender(<AccountLibraryPager t={t} libraryKey="empty" accounts={[]}
      maxAccountCount={18} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
  });

  it("updates the reserved rows when width changes, including on an empty tab", () => {
    const view = render(<AccountLibraryPager t={t} libraryKey="empty" accounts={[]}
      maxAccountCount={2} renderAccount={renderAccount} />);
    width = 300;
    notifyResize();
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
    width = 1_200;
    notifyResize();
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    view.unmount();
    expect(disconnect).toHaveBeenCalledOnce();
  });

  it("uses actual CSS tracks rather than overestimating columns before the scrollbar gutter", () => {
    const original = window.getComputedStyle;
    vi.spyOn(window, "getComputedStyle").mockImplementation((element, pseudo) => {
      const style = original(element, pseudo);
      if (element.classList.contains("nt-provider-account-card-grid")) {
        Object.defineProperty(style, "gridTemplateColumns", { value: "292px 292px 292px" });
      }
      return style;
    });
    render(<AccountLibraryPager t={t} libraryKey="empty" accounts={[]}
      maxAccountCount={4} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
  });

  it("resets internal scroll on tab changes without recreating the observer", () => {
    const view = render(<AccountLibraryPager t={t} libraryKey="many" accounts={accounts} renderAccount={renderAccount} />);
    pager().scrollTop = 400;
    view.rerender(<AccountLibraryPager t={t} libraryKey="other" accounts={accounts} renderAccount={renderAccount} />);
    expect(pager().scrollTop).toBe(0);
    expect(disconnect).not.toHaveBeenCalled();
    view.unmount();
    expect(disconnect).toHaveBeenCalledOnce();
  });

  it("recalculates when the largest tab count changes", () => {
    const view = render(<AccountLibraryPager t={t} libraryKey="empty" accounts={[]}
      maxAccountCount={10} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
    view.rerender(<AccountLibraryPager t={t} libraryKey="empty" accounts={[]}
      maxAccountCount={1} renderAccount={renderAccount} />);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
  });
});
