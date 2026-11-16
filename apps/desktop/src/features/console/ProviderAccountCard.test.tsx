import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { NeuroProvider } from "@neuro/ui";
import {
  ProviderAccountCard,
  type AccountCardActionId,
  type AccountsLedgerPilotAccount,
  type ProviderAccountCardHandlers,
} from "./ProviderAccountCard";

const t = (zh: string, _en: string) => zh;

beforeAll(() => {
  vi.stubGlobal(
    "ResizeObserver",
    class {
      observe() { }
      unobserve() { }
      disconnect() { }
    },
  );
});

afterAll(() => {
  vi.unstubAllGlobals();
});

function account(
  overrides: Partial<AccountsLedgerPilotAccount> = {},
): AccountsLedgerPilotAccount {
  return {
    accountId: "account-1",
    providerId: "provider-1",
    displayName: "Account 1",
    mode: "credential",
    enabled: true,
    logicalLabels: [],
    logicalGroupIds: [],
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
    ...overrides,
  };
}

function CardHarness(props: {
  account: AccountsLedgerPilotAccount;
  handlers: ProviderAccountCardHandlers;
  onAccountAction?: (
    action: AccountCardActionId,
    providerId: string,
    account: AccountsLedgerPilotAccount,
  ) => void;
}) {
  return (
    <NeuroProvider>
      <ProviderAccountCard
        t={t}
        account={props.account}
        editorLocked={false}
        groupOptions={[]}
        handlers={props.handlers}
        onAction={props.onAccountAction ?? vi.fn()}
      />
    </NeuroProvider>
  );
}

function handlers(
  overrides: Partial<ProviderAccountCardHandlers> = {},
): ProviderAccountCardHandlers {
  return {
    onEdit: vi.fn(),
    onRequestRemoval: vi.fn(),
    onAddExplicit: vi.fn(),
    onToggleDispatch: vi.fn(),
    onSetAccountGroup: vi.fn(),
    onOpenStats: vi.fn(),
    ...overrides,
  };
}

describe("ProviderAccountCard", () => {
  it("keeps discovery inside the account editor rather than the card", () => {
    const onAccountAction = vi.fn();
    const view = render(<CardHarness account={account({ discoverySupported: true })} handlers={handlers()} onAccountAction={onAccountAction} />);
    expect(screen.queryByRole("button", { name: "刷新模型和协议 Account 1" })).not.toBeInTheDocument();
    view.rerender(<CardHarness account={account({ discoverySupported: false })} handlers={handlers()} />);
    expect(screen.queryByRole("button", { name: "刷新模型和协议 Account 1" })).not.toBeInTheDocument();
  });
  it("shows only account creation actions for a provider-default account", async () => {
    const user = userEvent.setup();
    const onAddExplicit = vi.fn();
    render(
      <CardHarness
        account={account({ mode: "provider-default" })}
        handlers={handlers({ onAddExplicit })}
      />,
    );

    await user.click(screen.getByRole("button", { name: "添加显式账号" }));
    expect(onAddExplicit).toHaveBeenCalledWith("provider-1");
    expect(screen.queryByRole("switch", { name: "调度 account-1" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "编辑账号 Account 1" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除账号 Account 1" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "更多操作 account-1" })).not.toBeInTheDocument();
  });

  it("exposes four direct actions and an independent header edit button", async () => {
    const user = userEvent.setup();
    const onAccountAction = vi.fn();
    const onEdit = vi.fn();
    render(<CardHarness account={account()} handlers={handlers({ onEdit })} onAccountAction={onAccountAction} />);
    const footer = screen.getByRole("switch").closest("footer")!;
    expect(footer.querySelectorAll('button, input[role="switch"]')).toHaveLength(4);
    expect(screen.queryByRole("button", { name: "更多操作 account-1" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "查看 Account 1 统计" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "测试账号 Account 1" }));
    expect(onAccountAction).toHaveBeenCalledWith("probe", "provider-1", expect.objectContaining({ accountId: "account-1" }));
    await user.click(screen.getByRole("button", { name: "模型映射 Account 1" }));
    expect(onAccountAction).toHaveBeenCalledWith("model-mapping", "provider-1", expect.objectContaining({ accountId: "account-1" }));
    await user.click(screen.getByRole("button", { name: "编辑账号 Account 1" }));
    expect(onEdit).toHaveBeenCalledWith("provider-1", "account-1");
  });
});
