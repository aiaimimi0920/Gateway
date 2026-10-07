import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ProviderAccountLibrary } from "./ProviderAccountLibrary";
import { pilotAccount, pilotSection } from "./AccountsLedgerWorkspace.fixtures";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection } from "./accountsLedgerTypes";

const t = (zh: string, _en: string) => zh;
const rows = () => screen.getByRole("region", { name: "账号库滚动区域" }).querySelector("[data-account-library-rows]");
const accounts = (count: number, libraryName = "free") => Array.from({ length: count }, (_, index) =>
  pilotAccount({ accountId: `${libraryName}-${index}`, libraryName }));
const renderLibrary = (section: AccountsLedgerPilotSection, providerAccounts: AccountsLedgerPilotAccount[]) => render(
  <ProviderAccountLibrary section={section} providerAccounts={providerAccounts} t={t} editorLocked={false}
    onOpenGeminiManualAdd={vi.fn()} onAddExplicit={vi.fn()}
    renderAccount={(account) => <article key={account.accountId} data-account-card={account.accountId}>{account.displayName}</article>} />,
);
const chatgptSection = (free: AccountsLedgerPilotAccount[], plus: AccountsLedgerPilotAccount[] = []) => pilotSection({
  providerPreset: "chatgpt-codex-oauth-official-api", protocolProfile: "chatgpt_codex_backend", directAccounts: [],
  identityCategories: [free, plus].map((entries, index) => ({
    id: index ? "plus" : "free", label: index ? "plus" : "free", count: entries.length,
    poolTargetSize: 5, autoRefillEnabled: false, autoPruneEnabled: false, accounts: entries,
  })),
});

beforeEach(() => {
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 1_200, height: 0, top: 0, right: 1_200, bottom: 0, left: 0, x: 0, y: 0, toJSON: () => ({}),
  });
});
afterEach(() => vi.restoreAllMocks());

describe("ProviderAccountLibrary shared tab height", () => {
  it("keeps a one-row ChatGPT library stable when switching to plus with zero accounts", async () => {
    const free = accounts(2);
    renderLibrary(chatgptSection(free), free);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    await userEvent.click(screen.getByRole("tab", { name: "plus 0" }));
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    expect(screen.getByText("当前账号库为空")).toBeInTheDocument();
  });

  it("reserves two rows from the largest tab even when the initial tab is empty", async () => {
    const plus = accounts(10, "plus");
    renderLibrary(chatgptSection([], plus), plus);
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
    await userEvent.click(screen.getByRole("tab", { name: "plus 10" }));
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
    expect(document.querySelectorAll("[data-account-card]")).toHaveLength(10);
    await userEvent.click(screen.getByRole("tab", { name: "pro 0" }));
    expect(rows()).toHaveAttribute("data-account-library-rows", "2");
  });

  it("uses the maximum subgroup count, not the sum of all subgroup accounts", async () => {
    const entries = [...accounts(3, "alpha"), ...accounts(3, "beta")];
    renderLibrary(pilotSection({ directAccounts: entries }), entries);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    await userEvent.click(screen.getByRole("tab", { name: "beta 3" }));
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
  });

  it("reserves one row when a non-ChatGPT library has no subgroups or accounts", () => {
    renderLibrary(pilotSection({ directAccounts: [] }), []);
    expect(rows()).toHaveAttribute("data-account-library-rows", "1");
    expect(screen.getByText("当前账号库为空")).toBeInTheDocument();
  });
});
