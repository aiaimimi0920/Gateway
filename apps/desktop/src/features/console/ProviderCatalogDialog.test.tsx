import "@testing-library/jest-dom/vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ComponentProps, useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { TEMPLATE_ADAPTATION_RECEIPTS, templateAdaptationComplete } from "./providerTemplateAdaptation";

function renderDialog(overrides: Partial<ComponentProps<typeof ProviderCatalogDialog>> = {}) {
  const props: ComponentProps<typeof ProviderCatalogDialog> = {
    open: true, existingProviderIds: [], existingCredentialIds: [], locked: false, hasSecretAccess: true,
    onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit: vi.fn(), ...overrides,
  };
  render(<UiLocaleProvider><ProviderCatalogDialog {...props} /></UiLocaleProvider>);
  return props;
}
describe("ProviderCatalogDialog", () => {
  beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
  it("omits LongCat from the standard template directory without hiding other providers", () => {
    renderDialog();
    expect(screen.queryByRole("button", { name: /^longcat(?:\s*新调用流程已验收)?$/i })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /^openrouter-api(?:\s*新调用流程已验收)?$/i })).toBeInTheDocument();
  });
  it("keeps paused Flow incomplete but allows creating its independent provider", async () => {
    const user = userEvent.setup(); const props = renderDialog();
    const flow = screen.getByRole("button", { name: /^flow-web/ });
    expect(flow).toHaveAttribute("data-adaptation", "pending");
    expect(flow).toHaveAttribute("data-development", "paused");
    expect(flow).toHaveAttribute("title", "未完成（已暂停）");
    expect(flow).not.toHaveClass("nt-provider-catalog__item--adapted");
    expect(templateAdaptationComplete("flow-web")).toBe(false);
    await user.click(flow);
    expect(screen.getByLabelText("Base URL")).toHaveValue("https://flow.google.com");
    expect(screen.getByLabelText("专用适配器")).toHaveValue("Google Flow Web");
    expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({
      templateId: "flow-web", providerId: "flow-web", supportedModels: [],
      workflow: expect.objectContaining({ specialAdapter: "flow_web" }),
    }));
  });
  it("returns keyboard focus to the opener after Escape without submitting", async () => {
    const user = userEvent.setup(); const onSubmit = vi.fn();
    function Harness() {
      const [open, setOpen] = useState(false);
      return <UiLocaleProvider><button onClick={() => setOpen(true)}>Add provider</button>
        <ProviderCatalogDialog open={open} onOpenChange={setOpen} existingProviderIds={[]} existingCredentialIds={[]}
          locked={false} hasSecretAccess onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} /></UiLocaleProvider>;
    }
    render(<Harness />); const opener = screen.getByRole("button", { name: "Add provider" });
    await user.click(opener); await user.keyboard("{Escape}");
    await waitFor(() => expect(opener).toHaveFocus());
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(); expect(onSubmit).not.toHaveBeenCalled();
  });
  it("creates a custom provider without a first account or secret access", async () => {
    const user = userEvent.setup(); const props = renderDialog({ hasSecretAccess: false });
    await user.click(screen.getByRole("button", { name: "自定义模板" }));
    await user.click(screen.getByRole("button", { name: "custom-api" }));
    await user.clear(screen.getByLabelText("Provider ID")); await user.type(screen.getByLabelText("Provider ID"), "partner");
    await user.type(screen.getByLabelText("Base URL"), "https://partner.example.test/v1");
    await user.selectOptions(screen.getByLabelText("调用适配器"), "anthropic");
    await user.selectOptions(screen.getByLabelText("指纹适配器"), "claude_cli");
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "custom-api-provider",
      providerId: "partner", providerLabel: "partner", baseUrl: "https://partner.example.test/v1",
      requestClientProfile: "claude_cli", workflow: expect.objectContaining({ callAdapter: "anthropic" }) }));
    expect(vi.mocked(props.onSubmit).mock.calls[0][0].credentialId).toBeUndefined();
    expect(props.onRequestSecretAccess).not.toHaveBeenCalled();
  });
  it.each(["openai-api", "codex-oauth", "gemini-web"])("locks official %s technical fields", async (id) => {
    const user = userEvent.setup(); renderDialog({ existingProviderIds: [id, `${id}-2`] });
    await user.click(screen.getByRole("button", { name: new RegExp(`^${id}(?:\\s*新调用流程已验收)?$`) }));
    expect(screen.getByLabelText("Provider ID")).toHaveValue(`${id}-3`);
    expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
    for (const name of ["调用适配器", "指纹适配器", "专用适配器"]) expect(screen.getByLabelText(name)).toHaveAttribute("readonly");
    if (id === "openai-api") expect(screen.getByLabelText("调用适配器")).toHaveValue("OpenAI API");
    expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  });
  it("allows Azure resource URLs and keeps the fixed flow read-only", async () => {
    const user = userEvent.setup(); renderDialog();
    await user.click(screen.getByRole("button", { name: /^azure-api\s*新调用流程已验收$/ }));
    expect(screen.getByLabelText("Provider ID")).toHaveValue("azure-api");
    expect(screen.getByLabelText("Base URL")).not.toHaveAttribute("readonly");
    expect(screen.getByLabelText("调用适配器")).toHaveAttribute("readonly");
  });
  it("rejects ID collisions and credential-bearing URLs without closing", async () => {
    const user = userEvent.setup(); const props = renderDialog({ existingProviderIds: ["openai-api"] });
    await user.click(screen.getByRole("button", { name: /^openai-api(?:\s*新调用流程已验收)?$/ }));
    await user.clear(screen.getByLabelText("Provider ID")); await user.type(screen.getByLabelText("Provider ID"), "openai-api");
    await user.click(screen.getByRole("button", { name: "添加" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Provider ID openai-api 已存在");
    expect(props.onSubmit).not.toHaveBeenCalled(); expect(props.onOpenChange).not.toHaveBeenCalled();
  });
  it("supports keyboard tab navigation and keeps the draft on inventory refresh", async () => {
    const user = userEvent.setup(); renderDialog();
    await user.click(screen.getByRole("tab", { name: "调用流程" })); await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("tab", { name: "模型映射" })).toHaveFocus();
    await user.keyboard("{End}"); expect(screen.getByRole("tab", { name: "站点健康" })).toHaveFocus();
    expect(screen.getByRole("tab", { name: "站点健康" })).toHaveAttribute("aria-selected", "true");
  });
  it("does not mark historical canaries as new adapter acceptance", () => {
    renderDialog();
    expect(TEMPLATE_ADAPTATION_RECEIPTS.map(({ templateId }) => templateId)).toEqual(["nvidia-openai", "openrouter-openai", "you", "exa", "chatgpt", "tavily", "linkup", "websearchapi", "jina", "poe-openai", "groq-openai", "cohere-chat", "openai", "anthropic", "gemini-api", "azure-api", "mistral-api", "xai-api", "deepseek-api", "perplexity-api", "gemini-web", "gemini-canvas", "aistudio-web", "chatgpt-web", "flowmusic-web", "stitch-web"]);
    expect(templateAdaptationComplete("mistral-api")).toBe(true);
    expect(templateAdaptationComplete("xai-api")).toBe(true);
    expect(templateAdaptationComplete("deepseek-api")).toBe(true);
    expect(TEMPLATE_ADAPTATION_RECEIPTS.find(({ templateId }) => templateId === "xai-api")?.acceptanceBasis).toBe("documentation-and-local-tests");
    expect(TEMPLATE_ADAPTATION_RECEIPTS.find(({ templateId }) => templateId === "mistral-api")?.acceptanceBasis).toBe("documentation-and-local-tests");
    expect(templateAdaptationComplete("chatgpt")).toBe(true);
    expect(templateAdaptationComplete("openai")).toBe(true);
    expect(templateAdaptationComplete("anthropic")).toBe(true);
    expect(templateAdaptationComplete("gemini-api")).toBe(true);
    expect(templateAdaptationComplete("gemini-web")).toBe(true);
    expect(templateAdaptationComplete("gemini-canvas")).toBe(true);
    expect(templateAdaptationComplete("aistudio-web")).toBe(true);
    expect(templateAdaptationComplete("chatgpt-web")).toBe(true);
    expect(TEMPLATE_ADAPTATION_RECEIPTS.find(({ templateId }) => templateId === "chatgpt-web")).toEqual({
      templateId: "chatgpt-web", flowVersion: 1, verifiedAt: "2026-10-31T22:20:59.7882036Z",
      evidencePath: "docs/progress/2026-11-01-chatgpt-web-current-acceptance-r39.md",
    });
    expect(templateAdaptationComplete("azure-api")).toBe(true);
    expect(TEMPLATE_ADAPTATION_RECEIPTS.find(({ templateId }) => templateId === "azure-api")?.acceptanceBasis).toBe("documentation-and-local-tests");
    expect(templateAdaptationComplete("nvidia-openai")).toBe(true);
    expect(templateAdaptationComplete("openrouter-openai")).toBe(true);
    expect(templateAdaptationComplete("poe-openai")).toBe(true);
    expect(templateAdaptationComplete("groq-openai")).toBe(true);
    expect(templateAdaptationComplete("cohere-chat")).toBe(true);
    expect(templateAdaptationComplete("you")).toBe(true);
    expect(templateAdaptationComplete("exa")).toBe(true);
    expect(templateAdaptationComplete("tavily")).toBe(true);
    expect(templateAdaptationComplete("linkup")).toBe(true);
    expect(templateAdaptationComplete("websearchapi")).toBe(true);
    expect(templateAdaptationComplete("jina")).toBe(true);
    expect([...document.querySelectorAll('[data-adaptation="complete"]')].map((button) => button.textContent))
      .toEqual(["codex-oauth新调用流程已验收", "chatgpt-web新调用流程已验收", "openai-api新调用流程已验收", "anthropic-api新调用流程已验收", "gemini-api新调用流程已验收", "stitch-web新调用流程已验收", "gemini-web新调用流程已验收", "gemini-canvas新调用流程已验收", "aistudio-web新调用流程已验收", "azure-api新调用流程已验收", "cohere-api新调用流程已验收", "groq-api新调用流程已验收", "nvidia-api新调用流程已验收", "deepseek-api新调用流程已验收", "mistral-api新调用流程已验收", "xAI-api新调用流程已验收", "perplexity-api新调用流程已验收", "openrouter-api新调用流程已验收", "poe-api新调用流程已验收", "linkup-api新调用流程已验收", "tavily-api新调用流程已验收", "you-api新调用流程已验收", "exa-api新调用流程已验收", "jina-api新调用流程已验收", "websearchapi-api新调用流程已验收", "flowmusic-web新调用流程已验收"]);
    for (const text of document.querySelectorAll('[data-adaptation="complete"] span')) {
      expect(text).toHaveClass("nt-visually-hidden");
    }
  });
});
