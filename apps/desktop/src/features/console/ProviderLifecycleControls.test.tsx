import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { NeuroProvider } from "@neuro/ui";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProviderStorageEndpoints } from "./ProviderStorageEndpoints";
import { ProviderLifecycleField } from "./ProviderLifecycleField";
import { AccountsLedgerWorkspace } from "./AccountsLedgerWorkspace";
import { workspaceProps } from "./AccountsLedgerWorkspace.fixtures";
import { pushAppToast } from "../../components/AppToast";

vi.mock("../../components/AppToast", () => ({ pushAppToast: vi.fn() }));
const t = (_zh: string, en: string) => en;
beforeEach(() => vi.clearAllMocks());

describe("standard lifecycle controls", () => {
  it("returns a disabled native policy opener to the visible card after cancellation", async () => {
    const user = userEvent.setup();
    const props = workspaceProps();
    const { rerender } = render(<NeuroProvider><AccountsLedgerWorkspace {...props} /></NeuroProvider>);
    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /^Managed OpenAI/ }));
    await user.click(screen.getByRole("button", { name: "服务商设置" }));
    const opener = screen.getByRole("switch", { name: "Managed OpenAI 彻底删除模式" });
    await user.click(opener);
    rerender(<NeuroProvider><AccountsLedgerWorkspace {...props} editorLocked /></NeuroProvider>);
    expect(opener).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(screen.getByRole("dialog", { name: "服务商设置" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "取消" })).not.toBeInTheDocument();
    expect(props.onToggleProviderPermanentDelete).not.toHaveBeenCalled();
  });

  it("keeps native switch DOM and focus through a controlled policy update", async () => {
    const user = userEvent.setup();
    const onToggleProviderAutoRefill = vi.fn();
    const props = workspaceProps({ onToggleProviderAutoRefill });
    const { rerender } = render(<NeuroProvider><AccountsLedgerWorkspace {...props} /></NeuroProvider>);
    await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /^Managed OpenAI/ }));
    await user.click(screen.getByRole("button", { name: "服务商设置" }));
    const toggle = screen.getByRole("switch", { name: "Managed OpenAI 自动补号" });
    expect(toggle.tagName).toBe("INPUT");
    expect(toggle.closest("label")).toHaveClass("n-switch");
    await user.click(toggle);
    expect(onToggleProviderAutoRefill).toHaveBeenCalledExactlyOnceWith("managed-provider", true);
    rerender(<NeuroProvider><AccountsLedgerWorkspace {...props}
      pilotSections={props.pilotSections.map(section => ({ ...section, autoRefillEnabled: true }))} /></NeuroProvider>);
    expect(screen.getByRole("switch", { name: "Managed OpenAI 自动补号" })).toBe(toggle);
    expect(toggle).toBeChecked();
    expect(toggle).toHaveFocus();
  });

  it("associates inline validation and keeps the input through background lock updates", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const props = { providerLabel: "Pool", label: "Minimum", value: "1", numeric: true, maximum: 10, disabled: false, onSave, t };
    const { rerender } = render(<ProviderLifecycleField {...props} />);
    await user.click(screen.getByRole("button", { name: "Edit Pool Minimum" }));
    const input = screen.getByRole("spinbutton");
    await user.clear(input); await user.type(input, "11{Enter}");
    expect(input).toHaveClass("n-input", "n-compact");
    expect(input).toHaveAccessibleDescription("Enter a safe integer between 0 and 10.");
    expect(screen.getByRole("alert")).toHaveClass("n-notice");
    rerender(<ProviderLifecycleField {...props} disabled />);
    expect(screen.getByRole("spinbutton")).toBe(input);
    expect(input).toHaveValue(11);
    await user.click(screen.getByRole("button", { name: "Cancel editing Pool Minimum" }));
    expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();
    rerender(<ProviderLifecycleField {...props} />);
    await user.click(screen.getByRole("button", { name: "Edit Pool Minimum" }));
    expect(screen.getByRole("spinbutton")).toHaveValue(1);
  });

  it("copies the complete endpoint and reports success only after the clipboard resolves", async () => {
    const user = userEvent.setup();
    let resolve!: () => void;
    const write = vi.spyOn(navigator.clipboard, "writeText").mockReturnValue(new Promise<void>(done => { resolve = done; }));
    const endpoint = `https://example.test/${"long-endpoint/".repeat(20)}`;
    render(<ProviderStorageEndpoints providerLabel="Pool" label="Inquiry" value={endpoint} t={t} />);
    const copy = screen.getByRole("button", { name: "Copy Pool Inquiry API" });
    expect(copy).toHaveClass("n-icon-button");
    await user.click(copy);
    expect(write).toHaveBeenCalledExactlyOnceWith(endpoint);
    expect(pushAppToast).not.toHaveBeenCalled();
    await act(async () => resolve());
    expect(pushAppToast).toHaveBeenCalledExactlyOnceWith("success", "Inquiry copied.");
  });

  it("keeps clipboard failure visible and disables missing endpoints", async () => {
    const user = userEvent.setup();
    vi.spyOn(navigator.clipboard, "writeText").mockRejectedValue(new Error("denied"));
    const { rerender } = render(<ProviderStorageEndpoints providerLabel="Pool" label="Inquiry" value="https://example.test/" t={t} />);
    await user.click(screen.getByRole("button", { name: "Copy Pool Inquiry API" }));
    expect(pushAppToast).toHaveBeenCalledExactlyOnceWith("error", "Copy failed. Check clipboard permissions.");
    rerender(<ProviderStorageEndpoints providerLabel="Pool" label="Inquiry" t={t} />);
    expect(screen.getByRole("button", { name: "Copy Pool Inquiry API" })).toBeDisabled();
  });
});
