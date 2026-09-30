import "@testing-library/jest-dom/vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ComponentProps, useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";

function renderDialog(
  overrides: Partial<ComponentProps<typeof ProviderCatalogDialog>> = {},
) {
  const props: ComponentProps<typeof ProviderCatalogDialog> = {
    open: true,
    existingProviderIds: [],
    existingCredentialIds: [],
    locked: false,
    hasSecretAccess: true,
    onOpenChange: vi.fn(),
    onRequestSecretAccess: vi.fn(),
    onAddAccount: vi.fn(),
    onSubmit: vi.fn(),
    ...overrides,
  };
  render(
    <UiLocaleProvider>
      <ProviderCatalogDialog {...props} />
    </UiLocaleProvider>,
  );
  return props;
}

describe("ProviderCatalogDialog", () => {
  beforeEach(() => {
    window.localStorage.setItem("gateway-ui-locale", "zh-CN");
  });

  it("returns keyboard focus to the opener after Escape without submitting", async () => {
    const user = userEvent.setup();
    const onSubmit = vi.fn();
    function Harness() {
      const [open, setOpen] = useState(false);
      return <UiLocaleProvider>
        <button onClick={() => setOpen(true)}>Add provider</button>
        <ProviderCatalogDialog open={open} onOpenChange={setOpen}
          existingProviderIds={[]} existingCredentialIds={[]} locked={false}
          hasSecretAccess onRequestSecretAccess={vi.fn()} onAddAccount={vi.fn()}
          onSubmit={onSubmit} />
      </UiLocaleProvider>;
    }
    render(<Harness />);
    const opener = screen.getByRole("button", { name: "Add provider" });
    await user.click(opener);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(opener).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("creates a custom OpenAI-compatible provider draft with a first account and route models", async () => {
    const user = userEvent.setup();
    const props = renderDialog();

    await user.click(screen.getByRole("button", { name: /自定义 OpenAI-compatible/i }));
    await user.clear(screen.getByLabelText("Provider ID"));
    await user.type(screen.getByLabelText("Provider ID"), "partner-openai");
    await user.clear(screen.getByLabelText("显示名称"));
    await user.type(screen.getByLabelText("显示名称"), "Partner OpenAI");
    await user.clear(screen.getByLabelText("服务商标识"));
    await user.type(screen.getByLabelText("服务商标识"), "partner");
    await user.clear(screen.getByLabelText("服务商名称"));
    await user.type(screen.getByLabelText("服务商名称"), "Partner");
    await user.type(screen.getByLabelText("Base URL"), "https://partner.example.test/v1");
    await user.type(screen.getByLabelText("支持模型与聚合路由"), "partner-chat\npartner-reasoning");
    await user.clear(screen.getByLabelText("首个账号 ID"));
    await user.type(screen.getByLabelText("首个账号 ID"), "partner-account-1");
    await user.clear(screen.getByLabelText("账号名称"));
    await user.type(screen.getByLabelText("账号名称"), "Partner Account 1");
    await user.type(screen.getByLabelText("API Key"), "test-api-key");
    await user.click(screen.getByRole("button", { name: "创建服务商与首个账号" }));

    expect(props.onSubmit).toHaveBeenCalledWith({
      templateId: "custom-openai-compatible",
      providerId: "partner-openai",
      providerLabel: "Partner OpenAI",
      vendorKey: "partner",
      vendorName: "Partner",
      baseUrl: "https://partner.example.test/v1",
      supportedModels: ["partner-chat", "partner-reasoning"],
      credentialId: "partner-account-1",
      accountName: "Partner Account 1",
      apiKey: "test-api-key",
    });
  }, 15_000);

  it("routes an existing catalog provider into the normal add-account flow", async () => {
    const user = userEvent.setup();
    const props = renderDialog({ existingProviderIds: ["muyuan-openai"] });

    await user.click(screen.getByRole("button", { name: /Muyuan · 第三方 OpenAI 兼容/i }));
    expect(screen.getByText("该 Provider 已存在")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "为现有服务商添加账号" }));

    expect(props.onAddAccount).toHaveBeenCalledWith("muyuan-openai");
    expect(props.onSubmit).not.toHaveBeenCalled();
  });
});
