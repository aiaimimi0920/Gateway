import { Button, Dialog, Input, NeuroProvider } from "@neuro/ui";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { type ComponentProps, useState } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";

type Props = ComponentProps<typeof ProviderCatalogDialog>;
const defaults: Props = {
  open: true, existingProviderIds: [], existingCredentialIds: [], locked: false,
  hasSecretAccess: true, onOpenChange: () => { }, onRequestSecretAccess: () => { }, onSubmit: () => { }
};
function Tree(props: Partial<Props>) {
  return <NeuroProvider><UiLocaleProvider><ProviderCatalogDialog {...defaults} {...props} /></UiLocaleProvider></NeuroProvider>;
}
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
async function custom(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "自定义" }));
  await user.click(screen.getByRole("button", { name: "custom-api" }));
  await user.click(screen.getByRole("checkbox", { name: "同时添加首个账号" }));
  await user.type(screen.getByLabelText("Base URL"), "https://fixture.test/v1");
}

it("keeps the draft, field node and focus on background inventory refresh and revalidates collisions", async () => {
  const user = userEvent.setup(); const submit = vi.fn();
  const view = render(<Tree onSubmit={submit} />);
  const input = screen.getByLabelText("Provider ID");
  await user.clear(input); await user.type(input, "kept-draft");
  view.rerender(<Tree existingProviderIds={["kept-draft"]} onSubmit={submit} />);
  expect(screen.getByLabelText("Provider ID")).toBe(input);
  expect(input).toHaveValue("kept-draft"); expect(input).toHaveFocus();
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(screen.getByRole("alert")).toHaveTextContent("Provider ID kept-draft 已存在");
  expect(submit).not.toHaveBeenCalled(); expect(input).toBeInTheDocument();
});

it("retains hidden tab DOM, local mapping drafts and test editor state", async () => {
  const user = userEvent.setup(); render(<Tree />); await custom(user);
  await user.click(screen.getByRole("tab", { name: "模型映射" }));
  const source = screen.getByLabelText("自定义模型"); await user.type(source, "draft-model");
  await user.click(screen.getByRole("tab", { name: "自动测试" }));
  await user.click(screen.getByRole("button", { name: "配置测试集" }));
  const prompt = screen.getByLabelText("提示词"); await user.clear(prompt); await user.type(prompt, "Only fixture");
  await user.click(screen.getByRole("tab", { name: "模型映射" }));
  expect(screen.getByLabelText("自定义模型")).toBe(source); expect(source).toHaveValue("draft-model");
  await user.click(screen.getByRole("tab", { name: "自动测试" }));
  expect(screen.getByLabelText("提示词")).toBe(prompt); expect(prompt).toHaveValue("Only fixture");
  expect(document.querySelectorAll('[role="tabpanel"]')).toHaveLength(6);
  expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
});

it("uses stable mapping checkboxes and keeps association and clearing semantics", async () => {
  const user = userEvent.setup(); const submit = vi.fn(); render(<Tree onSubmit={submit} />); await custom(user);
  await user.click(screen.getByRole("tab", { name: "模型映射" }));
  await user.type(screen.getByLabelText("自定义模型"), "source"); await user.click(screen.getByRole("button", { name: "加入" }));
  await user.type(screen.getByLabelText("自定义目标"), "target"); await user.click(screen.getByRole("button", { name: "关联" }));
  const check = screen.getByRole("checkbox", { name: "target" }); expect(check).toBeChecked();
  await user.click(check); expect(check).not.toBeChecked(); expect(check).toHaveFocus();
  await user.keyboard(" "); expect(check).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "target" })).toBe(check);
  await user.click(screen.getByRole("button", { name: "清空关联" })); expect(check).not.toBeChecked();
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ workflow: expect.objectContaining({ mappings: { source: [] } }) }));
});

it("clears staged storage secrets on identity changes and still requests permission before submission", async () => {
  const user = userEvent.setup(); const submit = vi.fn(); const request = vi.fn();
  render(<Tree onSubmit={submit} onRequestSecretAccess={request} hasSecretAccess={false} />); await custom(user);
  await user.click(screen.getByRole("tab", { name: "账号池" }));
  await user.click(screen.getAllByRole("button", { name: "配置" })[0]);
  await user.selectOptions(screen.getByLabelText("连接方式"), "webdav");
  const paste = async (label: string, value: string) => { await user.click(screen.getByLabelText(label)); await user.paste(value); };
  await paste("服务地址", "https://storage.test");
  await paste("用户名", "fixture"); await paste("密码", "staged-only");
  await paste("服务地址", "/changed"); expect(screen.getByLabelText("密码")).toHaveValue("");
  await paste("密码", "new-staged");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(request).toHaveBeenCalledOnce(); expect(submit).not.toHaveBeenCalled();
  expect(screen.getByLabelText("密码")).toHaveValue("new-staged");
  await user.selectOptions(screen.getByLabelText("连接方式"), "local");
  await paste("目录路径", "C:/fixture");
  await user.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ secrets: {}, workflow: expect.objectContaining({ storage: { type: "local", path: "C:/fixture" } }) }));
});

it("locks editable controls without disabling cancellation and discards drafts on reopen", async () => {
  const user = userEvent.setup(); const change = vi.fn(); const view = render(<Tree onOpenChange={change} />); await custom(user);
  const input = screen.getByLabelText("Provider ID");
  view.rerender(<Tree locked onOpenChange={change} />); expect(input).toBeDisabled();
  expect(screen.getByRole("button", { name: "添加" })).toBeDisabled();
  await user.keyboard("{Escape}"); expect(change).toHaveBeenLastCalledWith(false);
  view.rerender(<Tree open={false} />); view.rerender(<Tree />);
  expect(screen.getByLabelText("Base URL")).not.toHaveValue("https://fixture.test/v1");
});

it("does not reclaim focus when a different dialog replaces the catalog", async () => {
  let replace = () => { };
  function Example() {
    const [catalog, setCatalog] = useState(false); const [other, setOther] = useState(false);
    replace = () => { setCatalog(false); setOther(true); };
    return <NeuroProvider><UiLocaleProvider><Button onClick={() => setCatalog(true)}>入口</Button>
      <ProviderCatalogDialog {...defaults} open={catalog} onOpenChange={setCatalog} />
      <Dialog title="其他弹窗" open={other} onOpenChange={setOther}><Input aria-label="其他输入" /></Dialog>
    </UiLocaleProvider></NeuroProvider>;
  }
  const user = userEvent.setup(); render(<Example />); await user.click(screen.getByRole("button", { name: "入口" }));
  act(replace);
  const other = await screen.findByRole("dialog", { name: "其他弹窗" });
  const close = within(other).getByRole("button", { name: "关闭" });
  await waitFor(() => expect(close).toHaveFocus());
  await new Promise(resolve => setTimeout(resolve, 20)); expect(close).toHaveFocus();
});
