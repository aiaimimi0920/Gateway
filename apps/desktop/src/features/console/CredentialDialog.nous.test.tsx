import { NeuroProvider } from "@neuro/ui";
import { render, fireEvent, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { CredentialDialog } from "./CredentialDialog";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it.each([true, false])("validates long model IDs in the actual submit path, Nous=%s", (nous) => {
  const onSubmit = vi.fn(); const model = "org/" + "x".repeat(400);
  render(<CredentialDialog open mode="edit" providerOptions={[{ id: "p", label: "Nous" }]} existingCredentialIds={["c"]}
    initialValue={{ providerId: "p", credentialId: "c", accountName: "", enabled: true, baseUrl: "", apiKeyOperation: "keep", apiKeyValue: "",
      supportedModelsText: model, fetchedModelsText: model, manualModelsText: "" }} locked={false} hasSecretAccess
    modelPolicy={() => nous ? "nous" : undefined} reauthenticationOnly={() => true}
    onOpenChange={vi.fn()} onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} />, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  if (nous) expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ supportedModelsText: model }));
  else expect(onSubmit).not.toHaveBeenCalled();
});
