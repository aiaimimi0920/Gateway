import { NeuroProvider } from "@neuro/ui";
import { useState } from "react";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { CredentialDialog } from "./CredentialDialog";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it("returns focus to the opener after Escape without submitting the OAuth or ordinary form", async () => {
  const onSubmit = vi.fn(); const user = userEvent.setup();
  function Harness() {
    const [open, setOpen] = useState(false);
    return <><button onClick={() => setOpen(true)}>打开 Accio</button>
      <CredentialDialog open={open} onOpenChange={setOpen} mode="add" providerOptions={[{ id: "accio", label: "Accio" }]}
        existingCredentialIds={[]} locked={false} hasSecretAccess onRequestSecretAccess={vi.fn()} onSubmit={onSubmit}
        renderAuthentication={() => <button type="button">通过 Accio 登录</button>} /></>;
  }
  render(<NeuroProvider><Harness /></NeuroProvider>); const opener = screen.getByRole("button", { name: /^打开 Accio$/ });
  await user.click(opener); expect(screen.getByRole("button", { name: "通过 Accio 登录" })).toHaveFocus();
  await user.keyboard("{Escape}");
  await waitFor(() => expect(opener).toHaveFocus());
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument(); expect(onSubmit).not.toHaveBeenCalled();
});
