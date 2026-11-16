import { NeuroProvider } from "@neuro/ui";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { expect, it, vi } from "vitest";
import { CredentialTestPlanFields } from "./CredentialTestPlanFields";
import { defaultTestPolicy } from "./credentialTestPolicyDocument";
import type { CredentialTestPolicy } from "../../api/contracts";

function setup(count = 2, initialCustom = false) {
  const changed = vi.fn();
  const configured = Array.from({ length: count }, (_, i) => `model-${i}`);
  function Harness() {
    const [policy, setPolicy] = useState<CredentialTestPolicy>({ ...defaultTestPolicy(), modelSelection: "selected", models: initialCustom ? ["custom"] : [] });
    return <CredentialTestPlanFields policy={policy} models={[...new Set([...configured, ...policy.models])]}
      configuredModels={configured} locked={false} t={(zh) => zh} onChange={(next) => { changed(next); setPolicy(next); }} />;
  }
  render(<Harness />, { wrapper: NeuroProvider });
  return changed;
}

it("keeps the last custom model candidate and its focus after deselection", async () => {
  const user = userEvent.setup(); const changed = setup(2, true);
  const custom = screen.getByRole("checkbox", { name: "custom" });
  await user.click(custom);
  expect(screen.getByRole("checkbox", { name: "custom" })).toBe(custom);
  expect(custom).not.toBeChecked(); expect(custom).toHaveFocus();
  expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({ modelSelection: "selected", models: [] }));
});

it("remembers explicitly added candidates after clearing and does not commit invalid pending input", async () => {
  const user = userEvent.setup(); const changed = setup();
  const input = screen.getByRole("textbox", { name: "添加测试模型" });
  await user.type(input, "new-model"); await user.click(screen.getByRole("button", { name: "添加模型" }));
  const custom = screen.getByRole("checkbox", { name: "new-model" });
  await user.click(screen.getByRole("button", { name: "清空选择" }));
  expect(screen.getByRole("checkbox", { name: "new-model" })).toBe(custom);
  expect(custom).not.toBeChecked();
  changed.mockClear(); await user.type(input, "*");
  expect(screen.getByRole("button", { name: "添加模型" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "添加模型" }));
  expect(changed).not.toHaveBeenCalled();
});

it("uses the standard searchable multiselect above six models without remounting an option", async () => {
  const user = userEvent.setup(); const changed = setup(8, true);
  await user.click(screen.getByRole("button", { name: /^选择模型，/ }));
  const custom = screen.getByRole("checkbox", { name: "custom" });
  await user.click(custom);
  expect(screen.getByRole("checkbox", { name: "custom" })).toBe(custom);
  expect(custom).toHaveFocus();
  await user.type(screen.getByRole("searchbox", { name: "搜索模型" }), "model-7");
  await user.click(screen.getByRole("checkbox", { name: "model-7" }));
  expect(changed).toHaveBeenLastCalledWith(expect.objectContaining({ models: ["model-7"] }));
  await user.keyboard("{Escape}");
  expect(screen.getByRole("button", { name: /^选择模型，/ })).toHaveFocus();
});

it("retains case detail DOM while toggling its checkbox", async () => {
  const user = userEvent.setup(); setup();
  const checkbox = screen.getByRole("checkbox", { name: /计算能力/ });
  const article = checkbox.closest("article");
  const disclosure = article?.querySelector("details");
  await user.click(checkbox);
  expect(checkbox.closest("article")).toBe(article);
  expect(article?.querySelector("details")).toBe(disclosure);
  expect(checkbox).toHaveFocus();
});

it("keeps the UTF-8 byte guard on temporary prompts rather than relying on maxlength", async () => {
  const user = userEvent.setup(); const changed = setup();
  const prompt = screen.getByRole("textbox", { name: "临时提示词" });
  await user.click(prompt); await user.paste("测".repeat(2800));
  await user.click(screen.getByRole("button", { name: "加入测试集" }));
  expect(screen.getByRole("alert")).toHaveTextContent("提示词或标准答案超过字节限制。");
  expect(changed).not.toHaveBeenCalled(); expect(prompt).toHaveValue("测".repeat(2800));
});
