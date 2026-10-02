import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ProviderLifecycleField } from "./ProviderLifecycleField";
import { providerStoragePathError } from "./providerStoragePath";
import { readPilotProviderPolicy } from "./pilotPoolPolicy";

const t = (zh: string) => zh;
const props = { providerLabel: "NVIDIA", label: "最小可用池", value: "1", numeric: true, disabled: false, t };

describe("provider lifecycle inline editing", () => {
  it("does not commit on blur, and discards cancelled numeric edits", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<ProviderLifecycleField {...props} onSave={onSave} />);
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    await user.clear(screen.getByRole("spinbutton"));
    await user.type(screen.getByRole("spinbutton"), "42");
    await user.tab();
    expect(onSave).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "取消编辑 NVIDIA 最小可用池" }));
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    expect(screen.getByRole("spinbutton")).toHaveValue(1);
    await user.keyboard("{Escape}");
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.queryByRole("spinbutton")).not.toBeInTheDocument();
  });

  it.each(["0", "1000000", String(Number.MAX_SAFE_INTEGER)])("accepts safe integer %s without a business cap", async (value) => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<ProviderLifecycleField {...props} onSave={onSave} />);
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    await user.clear(screen.getByRole("spinbutton"));
    await user.type(screen.getByRole("spinbutton"), value);
    await user.keyboard("{Enter}");
    expect(onSave).toHaveBeenCalledExactlyOnceWith(value);
  });

  it.each(["-1", "1.5", "9007199254740992", ""])('rejects invalid integer "%s"', async (value) => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<ProviderLifecycleField {...props} onSave={onSave} />);
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    await user.clear(screen.getByRole("spinbutton"));
    if (value) await user.type(screen.getByRole("spinbutton"), value);
    await user.keyboard("{Enter}");
    expect(onSave).not.toHaveBeenCalled();
    expect(screen.getByRole("alert")).toBeInTheDocument();
  });

  it("checks the cross-bound limit", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    render(<ProviderLifecycleField {...props} maximum={10} onSave={onSave} />);
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    await user.clear(screen.getByRole("spinbutton"));
    await user.type(screen.getByRole("spinbutton"), "11{Enter}");
    expect(onSave).not.toHaveBeenCalled();
  });

  it("preserves a rejected edit until cancellation and never retains secret drafts", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn(() => false);
    render(<ProviderLifecycleField {...props} numeric={false} label="存储密码" secret value="" configured onSave={onSave} />);
    expect(screen.getByText("••••")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 存储密码" }));
    const input = screen.getByLabelText("NVIDIA 存储密码");
    expect(input).toHaveAttribute("type", "password");
    await user.type(input, "test-only-secret{Enter}");
    expect(input).toHaveValue("test-only-secret");
    await user.keyboard("{Escape}");
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 存储密码" }));
    expect(screen.getByLabelText("NVIDIA 存储密码")).toHaveValue("");
  });

  it("keeps an unfinished edit through another field's autosave", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const view = render(<ProviderLifecycleField {...props} onSave={onSave} />);
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    await user.clear(screen.getByRole("spinbutton"));
    await user.type(screen.getByRole("spinbutton"), "42");
    view.rerender(<ProviderLifecycleField {...props} disabled onSave={onSave} />);
    expect(screen.getByRole("spinbutton")).toHaveValue(42);
    expect(screen.getByRole("spinbutton")).toBeDisabled();
    view.rerender(<ProviderLifecycleField {...props} onSave={onSave} />);
    expect(screen.getByRole("spinbutton")).toHaveValue(42);
    await user.click(screen.getByRole("button", { name: "保存 NVIDIA 最小可用池" }));
    expect(onSave).toHaveBeenCalledExactlyOnceWith("42");
  });

  it("clears the secret editor on permission loss", async () => {
    const user = userEvent.setup();
    const onSave = vi.fn();
    const view = render(<ProviderLifecycleField {...props} numeric={false} secret onSave={onSave} />);
    await user.click(screen.getByRole("button", { name: "编辑 NVIDIA 最小可用池" }));
    await user.type(screen.getByLabelText("NVIDIA 最小可用池"), "test-only-secret");
    view.rerender(<ProviderLifecycleField {...props} numeric={false} secret disabled discardDraft onSave={onSave} />);
    expect(screen.queryByDisplayValue("test-only-secret")).not.toBeInTheDocument();
    expect(onSave).not.toHaveBeenCalled();
  });
});

describe("provider capacity and path policy", () => {
  it("defaults max to 100 and min to 1 without changing saved values", () => {
    expect(readPilotProviderPolicy({})).toMatchObject({ poolTargetSize: 100, poolMinSize: 1 });
    expect(readPilotProviderPolicy({ pool_target_size: 42, pool_min_size: 0 })).toMatchObject({ poolTargetSize: 42, poolMinSize: 0 });
  });
  it.each(["https://storage.example.test/pool", "http://storage.example.test/pool"])("rejects undefined cloud protocol %s", (path) => {
    expect(providerStoragePathError(path)).toBe("cloud");
  });
  it.each(["/srv/gateway/pool", "C:\\Gateway\\pool", "\\\\server\\share\\pool"])("accepts a filesystem path %s", (path) => {
    expect(providerStoragePathError(path)).toBeNull();
  });
  it.each(["", "data:text/plain,test", "file:///etc/passwd", "a\nfile", "relative/path", "/tmp/../secrets"])("rejects invalid path %s", (path) => {
    expect(providerStoragePathError(path)).toBe("invalid");
  });
});
