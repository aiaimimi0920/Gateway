import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ModelPoolModelDialog, type ModelPoolModelDialogProps } from "./ModelPoolModelDialog";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "en-US"));
const options = [{ id: "a", label: "Alpha" }, { id: "b", label: "Beta" }];
function setup(overrides: Partial<ModelPoolModelDialogProps> = {}) {
  const props: ModelPoolModelDialogProps = {
    open: true, mode: "add", providerOptions: options, existingModels: ["existing"],
    locked: false, onOpenChange: vi.fn(), onSubmit: vi.fn(), ...overrides,
  };
  const wrapper = ({ children }: { children: React.ReactNode }) => <NeuroProvider><UiLocaleProvider>{children}</UiLocaleProvider></NeuroProvider>;
  return { ...render(<ModelPoolModelDialog {...props} />, { wrapper }), props, user: userEvent.setup() };
}

it("validates required, wildcard, collision and provider requirements before submitting the exact ordered value", async () => {
  const { user, props } = setup();
  const name = screen.getByRole("textbox", { name: "Model name" });
  await waitFor(() => expect(name).toHaveFocus());
  const submit = screen.getByRole("button", { name: "Add model" });
  await user.click(submit);
  expect(screen.getByRole("alert")).toHaveTextContent("Model name is required.");
  fireEvent.change(name, { target: { value: "bad*" } });
  await user.click(submit);
  expect(screen.getByRole("alert")).toHaveTextContent("cannot contain *");
  fireEvent.change(name, { target: { value: "existing" } });
  await user.click(submit);
  expect(screen.getByRole("alert")).toHaveTextContent("already exists");
  fireEvent.change(name, { target: { value: " new-model " } });
  await user.click(submit);
  expect(screen.getByRole("alert")).toHaveTextContent("Select at least one provider");
  expect(props.onSubmit).not.toHaveBeenCalled();
  await user.click(screen.getByRole("checkbox", { name: "Beta" }));
  await user.click(screen.getByRole("checkbox", { name: "Alpha" }));
  await user.click(submit);
  expect(props.onSubmit).toHaveBeenCalledExactlyOnceWith({ model: "new-model", providerIds: ["b", "a"] });
  expect(props.onOpenChange).toHaveBeenCalledWith(false);
});

it("keeps checkbox nodes and focus during selection, preserves ordered row nodes on reorder", async () => {
  const { user, container } = setup({ mode: "edit", initialValue: { model: "existing", providerIds: ["a"] } });
  const alpha = screen.getByRole("checkbox", { name: "Alpha" });
  const beta = screen.getByRole("checkbox", { name: "Beta" });
  const row = container.ownerDocument.querySelector(".nt-model-dialog__chain-row");
  await user.click(beta);
  expect(beta).toBeChecked(); expect(beta).toHaveFocus();
  expect(screen.getByRole("checkbox", { name: "Beta" })).toBe(beta);
  await user.click(screen.getByRole("button", { name: "Move Beta up one slot" }));
  expect(screen.getAllByRole("listitem")[1]).toBe(row);
  expect(screen.getByRole("button", { name: "Move Beta up one slot" })).toBeDisabled();
  await user.click(alpha);
  expect(alpha).not.toBeChecked(); expect(alpha).toHaveFocus();
  expect(screen.getByRole("checkbox", { name: "Alpha" })).toBe(alpha);
});

it("uses searchable multi-select for many choices, retains missing candidates and handles nested Escape", async () => {
  const many = Array.from({ length: 7 }, (_, index) => ({ id: `p${index}`, label: `Provider ${index}` }));
  const initialValue = { model: "existing", providerIds: ["missing", "p2"] };
  const { user, props, rerender } = setup({ mode: "edit", providerOptions: many, initialValue });
  const trigger = screen.getByRole("button", { name: /Providers，2 selected/ });
  await user.click(trigger);
  const missing = screen.getByRole("checkbox", { name: "missing" });
  await user.click(missing);
  expect(screen.getByRole("checkbox", { name: "missing" })).toBe(missing);
  expect(missing).not.toBeChecked(); expect(missing).toHaveFocus();
  await user.click(missing);
  await user.keyboard("{Escape}");
  expect(screen.getByRole("dialog")).toBeVisible(); expect(trigger).toHaveFocus();
  rerender(<ModelPoolModelDialog {...props} providerOptions={[...many].reverse()} />);
  await user.click(screen.getByRole("button", { name: "Save model" }));
  expect(props.onSubmit).toHaveBeenCalledExactlyOnceWith({ model: "existing", providerIds: ["p2", "missing"] });
});

it("disables edits after permission revocation but allows cancel and restores the external trigger", async () => {
  const submitted = vi.fn();
  function Harness() {
    const [open, setOpen] = useState(false);
    return <NeuroProvider><UiLocaleProvider><button onClick={() => setOpen(true)}>Open</button>
      <ModelPoolModelDialog open={open} mode="add" providerOptions={options} existingModels={[]} locked
        onOpenChange={setOpen} onSubmit={submitted} /></UiLocaleProvider></NeuroProvider>;
  }
  const user = userEvent.setup(); render(<Harness />);
  const trigger = screen.getByRole("button", { name: "Open" });
  await user.click(trigger);
  expect(screen.getByRole("textbox")).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: "Alpha" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Add model" })).toBeDisabled();
  await user.click(within(screen.getByRole("dialog")).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(trigger).toHaveFocus());
  expect(submitted).not.toHaveBeenCalled();
});
