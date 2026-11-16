import "@testing-library/jest-dom/vitest";
import { NeuroProvider } from "@neuro/ui";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("requires an explicit Qwen region and workspace before creating an empty provider", async () => {
  const user = userEvent.setup(); const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: /^qwen-api$/ }));
  expect(screen.getByLabelText("Qwen 地域")).toHaveValue("");
  expect(screen.getByLabelText("Base URL")).toHaveValue("");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled();
  await user.selectOptions(screen.getByLabelText("Qwen 地域"), "eu-central-1");
  await user.type(screen.getByLabelText("Qwen 工作空间"), "workspace-a");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({
    templateId: "qwen-api", supportedModels: [],
    baseUrl: "https://workspace-a.eu-central-1.maas.aliyuncs.com/compatible-mode/v1"
  }));
});
it("does not retain a previous Qwen target after selecting a different template and returning", async () => {
  const user = userEvent.setup(); const submit = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  await user.click(screen.getByRole("button", { name: /^qwen-api$/ }));
  await user.selectOptions(screen.getByLabelText("Qwen 地域"), "cn-beijing");
  await user.type(screen.getByLabelText("Qwen 工作空间"), "workspace-a");
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  await user.click(screen.getByRole("button", { name: /^bedrock-api$/ }));
  expect(screen.queryByLabelText("Qwen 地域")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: /^qwen-api$/ }));
  expect(screen.getByLabelText("Qwen 地域")).toHaveValue("");
  expect(screen.getByLabelText("Qwen 工作空间")).toHaveValue("");
  expect(screen.getByLabelText("Base URL")).toHaveValue("");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).not.toHaveBeenCalled();
});
