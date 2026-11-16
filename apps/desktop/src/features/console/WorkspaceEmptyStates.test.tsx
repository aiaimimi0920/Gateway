import { render, screen } from "@testing-library/react";
import { NeuroProvider } from "@neuro/ui";
import { expect, it, vi } from "vitest";
import { EntitlementGroupScopeBoard } from "./EntitlementGroupScopeBoard";
import { ModelPoolChainBoard } from "./ModelPoolChainBoard";
import { ModelPoolServingAccountsPanel } from "./ModelPoolServingAccountsPanel";

const t = (_zh: string, en: string) => en;

it.each([false, true])("keeps provider/model empty scope compact (provider present: %s)", (present) => {
  const change = vi.fn();
  const { container } = render(<EntitlementGroupScopeBoard t={t} cardKey="empty" label="Group"
    providerScopes={present ? [{ providerId: "p", providerLabel: "Provider", accountCount: 0, providerAccountCount: 0, modelCount: 0 }] : []}
    modelScopes={[]} deselectedProviderIds={[]} onDeselectedProviderIdsChange={change} />, { wrapper: NeuroProvider });
  expect(screen.getByText(present ? "Selected providers expose no models" : "Select providers to list models").closest(".n-empty")).toHaveClass("n-empty--compact");
  expect(container.querySelectorAll(".n-empty--compact")).toHaveLength(present ? 1 : 2);
  if (!present) expect(screen.getByText("No providers yet, attach accounts first")).toBeInTheDocument();
  expect(change).not.toHaveBeenCalled();
});

it("uses a compact empty chain without inventing a provider or triggering an action", () => {
  const change = vi.fn(), move = vi.fn();
  const { container } = render(<ModelPoolChainBoard t={t} cardKey="empty" model="model" chain={[]}
    editorLocked={false} deselectedProviderIds={[]} onDeselectedProviderIdsChange={change} onMoveProvider={move} />, { wrapper: NeuroProvider });
  expect(container.querySelector(".n-empty")).toHaveClass("n-empty--compact");
  expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
  expect(change).not.toHaveBeenCalled();
  expect(move).not.toHaveBeenCalled();
});

it.each([false, true])("preserves the serving-account scope explanation (narrowed: %s)", (narrowed) => {
  render(<ModelPoolServingAccountsPanel t={t} model="model" accountPanelId="empty" selectedLinks={[]}
    scopedAccountIds={[]} chainNarrowed={narrowed} editorLocked={false} />);
  const empty = screen.getByText(narrowed ? "No accounts fall inside the selected provider scope." : "No account serves this model yet.");
  expect(empty.closest(".n-empty")).toBeInTheDocument();
  expect(screen.getByRole("region", { name: "model serving accounts" })).toContainElement(empty);
});
