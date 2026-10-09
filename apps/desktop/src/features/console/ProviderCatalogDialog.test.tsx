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
          hasSecretAccess onRequestSecretAccess={vi.fn()}
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

    await user.click(screen.getByRole("button", { name: /自定义 API 服务商/i }));
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
      templateId: "custom-api-provider",
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

  it.each([
    ["muyuan-openai", /Muyuan · 第三方 OpenAI 兼容/i],
    ["openai", /OpenAI 官方 API/i],
    ["custom-api-provider", /自定义 API 服务商/i],
  ])("creates another independent %s provider with unoccupied identities", async (id, name) => {
    const user = userEvent.setup();
    const props = renderDialog({ existingProviderIds: [id, `${id}-2`], existingCredentialIds: [`${id}-3-account-1`] });
    await user.click(screen.getByRole("button", { name }));
    expect(screen.getByLabelText("Provider ID")).toHaveValue(`${id}-3`);
    expect(screen.getByLabelText("首个账号 ID")).toHaveValue(`${id}-3-account-1-2`);
    if (id === "custom-api-provider") {
      await user.type(screen.getByLabelText("Base URL"), "https://second.example.test");
      await user.type(screen.getByLabelText("支持模型与聚合路由"), "fixture-model");
    }
    await user.type(screen.getByLabelText("API Key"), "fixture-key");
    await user.click(screen.getByRole("button", { name: "创建服务商与首个账号" }));
    expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: `${id}-3`, credentialId: `${id}-3-account-1-2` }));
  });

  it("keeps a colliding ID editable and rejects it without redirecting or overwriting", async () => {
    const user = userEvent.setup();
    const props = renderDialog({ existingProviderIds: ["openai"] });
    await user.click(screen.getByRole("button", { name: /OpenAI 官方 API/i }));
    await user.clear(screen.getByLabelText("Provider ID"));
    await user.type(screen.getByLabelText("Provider ID"), "openai");
    await user.type(screen.getByLabelText("API Key"), "fixture-key");
    await user.click(screen.getByRole("button", { name: "创建服务商与首个账号" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Provider ID openai 已存在");
    expect(props.onSubmit).not.toHaveBeenCalled();
    expect(props.onOpenChange).not.toHaveBeenCalled();
    await user.clear(screen.getByLabelText("Provider ID"));
    await user.type(screen.getByLabelText("Provider ID"), "openai-second");
    await user.click(screen.getByRole("button", { name: "创建服务商与首个账号" }));
    expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "openai-second" }));
  });
});
