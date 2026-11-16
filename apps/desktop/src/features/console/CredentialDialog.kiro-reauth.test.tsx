import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it.each(["replace", "clear"] as const)("rejects stale or programmatic %s secret submission for reauthentication-only accounts", (operation) => {
  const props: CredentialDialogProps = {
    open: true, mode: "edit", providerOptions: [{ id: "kiro-pool", label: "Kiro" }], existingCredentialIds: ["stable-id"],
    initialValue: { providerId: "kiro-pool", credentialId: "stable-id", accountName: "Preserved", enabled: false,
      baseUrl: "", supportedModelsText: "", apiKeyOperation: operation, apiKeyValue: operation === "replace" ? "synthetic-token" : "" },
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onSubmit: vi.fn(), onRequestSecretAccess: vi.fn(),
    reauthenticationOnly: () => true,
  };
  render(<NeuroProvider><CredentialDialog {...props} /></NeuroProvider>);
  expect(screen.queryByLabelText("API Key")).not.toBeInTheDocument();
  fireEvent.submit(screen.getByLabelText("Provider").closest("form")!);
  expect(props.onSubmit).not.toHaveBeenCalled(); expect(props.onOpenChange).not.toHaveBeenCalled();
});

it("does not fall back to generic account creation if an authorization renderer is temporarily unavailable", () => {
  const props: CredentialDialogProps = {
    open: true, mode: "add", providerOptions: [{ id: "kiro-pool", label: "Kiro" }], existingCredentialIds: [],
    locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onSubmit: vi.fn(), onRequestSecretAccess: vi.fn(), reauthenticationOnly: () => true,
  };
  render(<NeuroProvider><CredentialDialog {...props} /></NeuroProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "not-an-oauth-credential" } });
  fireEvent.submit(screen.getByLabelText("Provider").closest("form")!); expect(props.onSubmit).not.toHaveBeenCalled();
});
