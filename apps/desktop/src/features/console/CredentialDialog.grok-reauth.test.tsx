import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const initialValue: NonNullable<CredentialDialogProps["initialValue"]> = {
  providerId: "custom-build", credentialId: "stable-id", accountName: "Preserved", enabled: false,
  baseUrl: "", supportedModelsText: "model-a", apiKeyOperation: "keep", apiKeyValue: ""
};
function setup() {
  const props: CredentialDialogProps = {
    open: true, mode: "edit", providerOptions: [{ id: "custom-build", label: "Build" }], existingCredentialIds: ["stable-id"],
    initialValue, locked: false, hasSecretAccess: true, onOpenChange: vi.fn(), onSubmit: vi.fn(), onRequestSecretAccess: vi.fn(),
    renderReauthentication: (provider, credential) => <div data-testid="reauth">{provider}:{credential}</div>
  };
  render(<NeuroProvider><CredentialDialog {...props} /></NeuroProvider>); return props;
}

it("keeps normal metadata editing available and reauthorizes only the selected stable account", () => {
  const props = setup();
  expect(screen.queryByTestId("reauth")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "重新授权此账号" }));
  expect(screen.getByTestId("reauth")).toHaveTextContent("custom-build:stable-id");
  expect(screen.queryByRole("button", { name: "保存" })).not.toBeInTheDocument();
  fireEvent.submit(screen.getByLabelText("Provider").closest("form")!); expect(props.onSubmit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "返回账号编辑" }));
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({ credentialId: "stable-id", accountName: "Preserved", enabled: false, apiKeyOperation: "keep" }));
});

it("requires local form changes to be saved or discarded before switching to reauthorization", () => {
  setup();
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "synthetic-new-key" } });
  expect(screen.getByRole("button", { name: "重新授权此账号" })).toBeDisabled();
  expect(screen.getByText("请先保存或放弃当前表单修改，再重新授权。")).toBeInTheDocument();
});
