import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { CredentialDialog, type CredentialDialogProps, type CredentialDialogValue } from "./CredentialDialog";
import { NeuroProvider } from "@neuro/ui";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const value: CredentialDialogValue = { providerId: "pool", credentialId: "saved", accountName: "", enabled: true,
  baseUrl: "", supportedModelsText: "", apiKeyOperation: "keep", apiKeyValue: "" };
const owner = (): CredentialDialogProps => ({ open: true, mode: "edit", providerOptions: [{ id: "pool", label: "Pool" }],
  existingCredentialIds: ["saved"], initialValue: value, locked: false, hasSecretAccess: true,
  onOpenChange: vi.fn(), onRequestSecretAccess: vi.fn(), onSubmit: vi.fn() });
it("uses the original stored identity and fences all local edits without submitting or mutating models", () => {
  const props = owner(); const tools = vi.fn((provider: string, credential: string, disabled: boolean) =>
    <button disabled={disabled}>{provider}/{credential} catalog</button>);
  render(<NeuroProvider><CredentialDialog {...props} renderStoredAccountTools={tools} /></NeuroProvider>);
  expect(screen.getByRole("button", { name: "pool/saved catalog" })).toBeEnabled();
  fireEvent.change(screen.getByLabelText("手动模型列表"), { target: { value: "unsaved" } });
  expect(screen.getByRole("button", { name: "pool/saved catalog" })).toBeDisabled(); expect(props.onSubmit).not.toHaveBeenCalled();
});
it.each([{ mode: "add" as const }, { open: false }])("never mounts stored tools outside an open edit: %j", (patch) => {
  const tools = vi.fn(); render(<NeuroProvider><CredentialDialog {...owner()} {...patch} renderStoredAccountTools={tools} /></NeuroProvider>);
  expect(tools).not.toHaveBeenCalled();
});
