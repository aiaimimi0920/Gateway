import "@testing-library/jest-dom/vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";

it("adds only a provider and keeps the right-hand draft when switching template groups", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup();
  const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>);
  expect(screen.getByRole("heading", { name: "添加服务商" })).toBeInTheDocument();
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("搜索服务商")).not.toBeInTheDocument();
  expect(screen.getAllByRole("tab")).toHaveLength(5);
  expect(screen.getByLabelText("调用适配器")).toBeInTheDocument();
  expect(screen.getByLabelText("指纹适配器")).toBeInTheDocument();
  expect(screen.getByLabelText("专用适配器")).toBeInTheDocument();
  await user.clear(screen.getByLabelText("Provider ID"));
  await user.type(screen.getByLabelText("Provider ID"), "my-provider");
  await user.click(screen.getByRole("tab", { name: "模型映射" }));
  await user.click(screen.getByRole("button", { name: "自定义模板" }));
  expect(screen.getByLabelText("Provider ID")).toHaveValue("my-provider");
  expect(screen.getByRole("tab", { name: "模型映射" })).toHaveAttribute("aria-selected", "true");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ providerId: "my-provider" }));
  expect(onSubmit.mock.calls[0][0].apiKey).toBeUndefined();
});

it("treats prototype-like model names as ordinary mapping keys", async () => {
  localStorage.setItem("gateway-ui-locale", "zh-CN");
  const user = userEvent.setup(); const onSubmit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />
  </UiLocaleProvider>);
  await user.click(screen.getByRole("button", { name: "自定义模板" }));
  await user.click(screen.getByRole("button", { name: "custom-api" }));
  await user.type(screen.getByLabelText("Base URL"), "https://fixture.test");
  await user.click(screen.getByRole("tab", { name: "模型映射" }));
  await user.type(screen.getByLabelText("自定义模型"), "__proto__");
  await user.click(screen.getByRole("button", { name: "加入" }));
  await user.type(screen.getByLabelText("自定义目标"), "constructor");
  await user.click(screen.getByRole("button", { name: "关联" }));
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(Object.entries(onSubmit.mock.calls[0][0].workflow.mappings)).toEqual([["__proto__", ["constructor"]]]);
});
