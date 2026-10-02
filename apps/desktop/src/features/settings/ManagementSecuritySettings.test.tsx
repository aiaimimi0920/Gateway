import { act, fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { deferred, renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { ManagementSecuritySettings } from "./ManagementSecuritySettings";

const bootstrapStatus = { needsBootstrap: false, managementConfigured: true, environmentOverride: false };

function setup(rotate = vi.fn().mockResolvedValue(undefined)) {
  renderWithProviders(<ManagementSecuritySettings />, { bootstrapStatus, rotate });
  return { rotate, user: userEvent.setup(), submit: screen.getByRole("button", { name: "更新管理密钥" }) };
}

describe("ManagementSecuritySettings", () => {
  beforeEach(() => { localStorage.clear(); sessionStorage.clear(); });

  it("requires matching new tokens and explicit acknowledgement, then clears secret inputs", async () => {
    const pending = deferred<void>();
    const { user, submit, rotate } = setup(vi.fn(() => pending.promise));
    const token = screen.getByLabelText("新管理密钥");
    const confirmation = screen.getByLabelText("再次输入新密钥");
    expect(token).toHaveAttribute("type", "password");
    expect(token).toHaveAttribute("maxlength", "4096");
    await user.type(token, "synthetic-replacement");
    await user.type(confirmation, "different");
    expect(screen.getByRole("alert")).toHaveTextContent("不一致");
    expect(submit).toBeDisabled();
    await user.clear(confirmation);
    await user.type(confirmation, "synthetic-replacement");
    expect(submit).toBeDisabled();
    await user.click(screen.getByRole("checkbox"));
    await user.click(submit);
    fireEvent.click(submit);
    expect(rotate).toHaveBeenCalledExactlyOnceWith("synthetic-replacement");
    expect(submit).toBeDisabled();
    await act(async () => pending.resolve());
    expect(await screen.findByRole("status")).toHaveTextContent("管理密钥已更新");
    expect(token).toHaveValue("");
    expect(confirmation).toHaveValue("");
    expect(screen.getByRole("checkbox")).not.toBeChecked();
    expect(JSON.stringify(localStorage)).not.toContain("synthetic-replacement");
  });

  it("does not submit the current token unchanged", async () => {
    const { user, submit, rotate } = setup();
    await user.type(screen.getByLabelText("新管理密钥"), "management-secret");
    await user.type(screen.getByLabelText("再次输入新密钥"), "management-secret");
    await user.click(screen.getByRole("checkbox"));
    expect(submit).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("与当前密钥不同");
    expect(rotate).not.toHaveBeenCalled();
  });

  it("rejects a replacement that cannot be used in HTTP headers", async () => {
    const { user, submit, rotate } = setup();
    await user.type(screen.getByLabelText("新管理密钥"), "中文密钥");
    await user.type(screen.getByLabelText("再次输入新密钥"), "中文密钥");
    await user.click(screen.getByRole("checkbox"));
    expect(submit).toBeDisabled();
    expect(screen.getByRole("alert")).toHaveTextContent("ASCII");
    expect(rotate).not.toHaveBeenCalled();
  });

  it("keeps an environment-controlled token read-only", () => {
    const rotate = vi.fn();
    renderWithProviders(<ManagementSecuritySettings />, {
      bootstrapStatus: { ...bootstrapStatus, environmentOverride: true }, rotate,
    });
    expect(screen.getByRole("status")).toHaveTextContent("环境变量托管");
    expect(screen.queryByLabelText("新管理密钥")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "更新管理密钥" })).not.toBeInTheDocument();
    expect(rotate).not.toHaveBeenCalled();
  });

  it("reports an unconfirmed update without echoing a secret from an error", async () => {
    const { user, submit } = setup(vi.fn().mockRejectedValue(new Error("sensitive-error-secret")));
    await user.type(screen.getByLabelText("新管理密钥"), "synthetic-replacement");
    await user.type(screen.getByLabelText("再次输入新密钥"), "synthetic-replacement");
    await user.click(screen.getByRole("checkbox"));
    await user.click(submit);
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("未能确认"));
    expect(document.body.textContent).not.toContain("sensitive-error-secret");
    expect(screen.getByLabelText("新管理密钥")).toHaveValue("synthetic-replacement");
  });
});
