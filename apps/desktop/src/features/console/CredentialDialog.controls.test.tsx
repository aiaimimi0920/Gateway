import { Button, Dialog, Field, Input, NeuroProvider } from "@neuro/ui";
import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";
import { CredentialSecretField } from "./CredentialSecretField";
import type { CatalogInput, OfficialModelCatalog } from "./officialModelCatalog";

const initialValue: NonNullable<CredentialDialogProps["initialValue"]> = {
  providerId: "api", credentialId: "account", accountName: "", enabled: false, baseUrl: "",
  supportedModelsText: "old-model", apiKeyOperation: "keep", apiKeyValue: ""
};
function Tree(overrides: Partial<CredentialDialogProps>) {
  return <NeuroProvider><UiLocaleProvider><CredentialDialog open mode="edit" providerOptions={[{ id: "api", label: "API" }]}
    existingCredentialIds={[]} initialValue={initialValue} locked={false} hasSecretAccess
    onOpenChange={() => { }} onRequestSecretAccess={() => { }} onSubmit={() => { }} {...overrides} /></UiLocaleProvider></NeuroProvider>;
}
beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("retains model draft nodes and focus when inventory refreshes and revalidates an add collision", async () => {
  const user = userEvent.setup(); const submit = vi.fn();
  const options = { mode: "add" as const, initialValue: { ...initialValue, credentialId: "new-account", apiKeyOperation: "replace" as const, apiKeyValue: "synthetic" }, onSubmit: submit };
  const view = render(<Tree {...options} />);
  const input = screen.getByLabelText("手动模型列表"); await user.type(input, "\nkept-model");
  view.rerender(<Tree {...options} providerOptions={[{ id: "api", label: "Refreshed API" }]} existingCredentialIds={["new-account"]} />);
  expect(screen.getByLabelText("手动模型列表")).toBe(input); expect(input).toHaveFocus(); expect(input).toHaveValue("old-model\nkept-model");
  await user.click(screen.getByRole("button", { name: "保存" }));
  expect(screen.getByRole("alert")).toHaveTextContent("已存在"); expect(submit).not.toHaveBeenCalled();
});

it("preserves disabled metadata and uses the externally associated save action", async () => {
  const user = userEvent.setup(); const submit = vi.fn(); render(<Tree onSubmit={submit} />);
  const save = screen.getByRole("button", { name: "保存" }) as HTMLButtonElement;
  expect(save.form).toBe(screen.getByLabelText("Provider").closest("form"));
  await user.click(save);
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ enabled: false, apiKeyOperation: "keep", supportedModelsText: "old-model" }));
});

it("allows cancellation while locked, returns focus, and clears the editing session", async () => {
  const user = userEvent.setup(); const submit = vi.fn();
  function Harness() {
    const [open, setOpen] = useState(false);
    return <NeuroProvider><UiLocaleProvider><Button onClick={() => setOpen(true)}>打开账号</Button>
      <CredentialDialog open={open} onOpenChange={setOpen} mode="edit" providerOptions={[{ id: "api", label: "API" }]}
        existingCredentialIds={[]} initialValue={initialValue} locked hasSecretAccess onRequestSecretAccess={() => { }} onSubmit={submit} />
    </UiLocaleProvider></NeuroProvider>;
  }
  render(<Harness />); const opener = screen.getByRole("button", { name: "打开账号" }); await user.click(opener);
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "取消" })).toBeEnabled(); await user.keyboard("{Escape}");
  await waitFor(() => expect(opener).toHaveFocus()); expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(submit).not.toHaveBeenCalled();
});

it("cancels an in-flight catalog on close and rejects its late success", async () => {
  const user = userEvent.setup(); let finish!: (value: OfficialModelCatalog) => void;
  const discover = vi.fn((_input: CatalogInput, _signal?: AbortSignal) => new Promise<OfficialModelCatalog>(resolve => { finish = resolve; }));
  const submit = vi.fn(); const options = { catalogProviderKind: () => "openai" as const, onDiscoverProviderModels: discover, onSubmit: submit };
  const view = render(<Tree {...options} />); await user.click(screen.getByRole("button", { name: "拉取模型" }));
  expect(screen.getByRole("button", { name: "取消" })).toBeEnabled();
  view.rerender(<Tree {...options} open={false} />); expect(discover.mock.calls[0][1]?.aborted).toBe(true);
  await act(async () => finish({ models: ["late-model"], sourceUrl: "https://api.openai.com/v1/models", checkedAt: "2026-11-04T00:00:00Z" }));
  view.rerender(<Tree {...options} />); expect(screen.getByLabelText("拉取模型列表")).toHaveValue(""); expect(submit).not.toHaveBeenCalled();
});

it("keeps an active permission dialog focused when the background account editor closes", async () => {
  const user = userEvent.setup(); let close!: () => void;
  function Harness() {
    const [open, setOpen] = useState(false), [permission, setPermission] = useState(false); close = () => setOpen(false);
    return <NeuroProvider><UiLocaleProvider><Button onClick={() => setOpen(true)}>打开账号</Button>
      <CredentialDialog open={open} onOpenChange={setOpen} mode="edit" providerOptions={[{ id: "api", label: "API" }]}
        existingCredentialIds={[]} initialValue={initialValue} locked={false} hasSecretAccess={false}
        onRequestSecretAccess={() => setPermission(true)} onSubmit={() => { }} />
      <Dialog title="权限确认" open={permission} onOpenChange={setPermission}><Field label="确认字段"><Input /></Field></Dialog>
    </UiLocaleProvider></NeuroProvider>;
  }
  render(<Harness />); const opener = screen.getByRole("button", { name: "打开账号" }); await user.click(opener);
  await user.click(screen.getByRole("button", { name: "显示 API Key" }));
  const permissionInput = screen.getByLabelText("确认字段"); await user.click(permissionInput);
  const focus = vi.spyOn(opener, "focus"); act(close);
  await waitFor(() => expect(screen.getAllByRole("dialog")).toHaveLength(1)); expect(permissionInput).toHaveFocus(); expect(focus).not.toHaveBeenCalled();
});

it("assigns distinct secret input labels across instances", () => {
  const props = {
    value: "", existing: false, disabled: false, hasSecretAccess: true,
    onRequestSecretAccess() { }, onLoaded() { }, onChange() { }
  };
  render(<UiLocaleProvider><CredentialSecretField {...props} label="First secret" /><CredentialSecretField {...props} label="Second secret" /></UiLocaleProvider>);
  const first = screen.getByLabelText("First secret"), second = screen.getByLabelText("Second secret");
  expect(first.id).not.toBe(second.id); expect(first).toHaveClass("n-input"); expect(second).toHaveClass("n-input");
});

it("keeps the reveal action and input while loading, and aborts on permission withdrawal", async () => {
  let signal!: AbortSignal; let finish!: (value: string) => void;
  const onLoaded = vi.fn();
  const props = {
    value: "", existing: true, disabled: false, hasSecretAccess: true, onRequestSecretAccess: vi.fn(), onLoaded, onChange: vi.fn(),
    onReveal: (next: AbortSignal) => { signal = next; return new Promise<string>(resolve => { finish = resolve; }); }
  };
  const view = render(<UiLocaleProvider><CredentialSecretField {...props} /></UiLocaleProvider>);
  const input = screen.getByLabelText("API Key"), button = screen.getByRole("button", { name: "显示 API Key" });
  await userEvent.click(button);
  expect(screen.getByRole("button", { name: "显示 API Key" })).toBe(button);
  expect(button).toHaveAttribute("aria-busy", "true"); expect(button).toBeDisabled();
  expect(screen.getByRole("status")).toHaveClass("n-sr-only");
  expect(screen.getByLabelText("API Key")).toBe(input);
  view.rerender(<UiLocaleProvider><CredentialSecretField {...props} hasSecretAccess={false} /></UiLocaleProvider>);
  expect(signal.aborted).toBe(true);
  await act(async () => finish("late-synthetic-secret"));
  expect(onLoaded).not.toHaveBeenCalled(); expect(input).toHaveAttribute("type", "password"); expect(input).toHaveValue("");
});

it("retains fetch and save action names during directory loading", async () => {
  let finish!: (value: OfficialModelCatalog) => void;
  render(<Tree catalogProviderKind={() => "openai"} onDiscoverProviderModels={() => new Promise(resolve => { finish = resolve; })} />);
  const fetch = screen.getByRole("button", { name: "拉取模型" }), save = screen.getByRole("button", { name: "保存" });
  await userEvent.click(fetch);
  expect(screen.getByRole("button", { name: "拉取模型" })).toBe(fetch);
  expect(screen.getByRole("button", { name: "保存" })).toBe(save);
  expect(fetch).toHaveAttribute("aria-busy", "true"); expect(save).toBeDisabled();
  await act(async () => finish({ models: [], sourceUrl: "https://api.openai.com/v1/models", checkedAt: "2026-11-05T00:00:00Z" }));
  expect(save).toBeEnabled();
});
