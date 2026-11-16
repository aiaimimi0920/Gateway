import { useState } from "react";
import { NeuroProvider } from "@neuro/ui";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { emptyKiroOAuthFields, KiroOAuthFields } from "./KiroOAuthFields";

it("uses five stable standard choices without choosing a scope or directory for the operator", async () => {
  localStorage.setItem("gateway-ui-locale", "en-US");
  const changed = vi.fn();
  function Example() {
    const [value, setValue] = useState(emptyKiroOAuthFields);
    return <KiroOAuthFields value={value} disabled={false} editing={false}
      onChange={next => { changed(next); setValue(next); }} />;
  }
  render(<NeuroProvider><UiLocaleProvider><Example /></UiLocaleProvider></NeuroProvider>);
  const directory = screen.getByRole("combobox", { name: "Authorization directory" });
  expect(directory).toHaveClass("n-select");
  expect(directory).toHaveValue("");
  expect(screen.getAllByRole("checkbox")).toHaveLength(5);
  for (const checkbox of screen.getAllByRole("checkbox")) expect(checkbox).not.toBeChecked();
  const machine = screen.getByRole("textbox", { name: "Machine ID" });
  const scope = screen.getByRole("checkbox", { name: "codewhisperer:conversations" });
  expect(scope.closest("label")).toHaveClass("n-check");
  await userEvent.click(scope);
  expect(screen.getByRole("checkbox", { name: "codewhisperer:conversations" })).toBe(scope);
  expect(scope).toHaveFocus();
  expect(screen.getByRole("textbox", { name: "Machine ID" })).toBe(machine);
  expect(changed).toHaveBeenLastCalledWith({ ...emptyKiroOAuthFields, scopes: ["codewhisperer:conversations"] });
  await userEvent.keyboard(" ");
  expect(changed).toHaveBeenLastCalledWith(emptyKiroOAuthFields);
});

it("keeps the selected controls mounted and disabled during authorization without adding a new Machine ID for reauthorization", () => {
  const value = { ...emptyKiroOAuthFields, authMethod: "identity_center" as const, directoryRegion: "eu-west-1", startUrl: "https://organization.awsapps.com/start", scopes: ["codewhisperer:conversations" as const] };
  const view = render(<NeuroProvider><UiLocaleProvider><KiroOAuthFields value={value} disabled={false} editing onChange={vi.fn()} /></UiLocaleProvider></NeuroProvider>);
  const directory = screen.getByRole("combobox");
  const scope = screen.getByRole("checkbox", { name: "codewhisperer:conversations" });
  view.rerender(<NeuroProvider><UiLocaleProvider><KiroOAuthFields value={value} disabled editing onChange={vi.fn()} /></UiLocaleProvider></NeuroProvider>);
  expect(screen.getByRole("combobox")).toBe(directory);
  expect(directory).toBeDisabled();
  expect(screen.getByRole("checkbox", { name: "codewhisperer:conversations" })).toBe(scope);
  expect(scope).toBeChecked();
  expect(scope).toBeDisabled();
  expect(screen.queryByRole("textbox", { name: "Machine ID" })).not.toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Directory Region" })).toHaveValue("eu-west-1");
});
