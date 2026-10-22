import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { RequestClientProfileSelect } from "./RequestClientProfileSelect";
import { readRequestClientProfile, writeRequestClientProfile } from "./requestClientProfile";
import { providerDefinitionFromCatalogDraft, PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { AccountsLedgerWorkspace } from "./AccountsLedgerWorkspace";
import { pilotSection, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";

it("defaults legacy values and preserves credentials and independent provider settings", () => {
  expect(readRequestClientProfile(undefined)).toBe("auto");
  expect(readRequestClientProfile("unknown")).toBe("auto");
  const provider = { id: "relay", api_key: "fixture", credentials: [{ id: "one" }], site_health: {} };
  writeRequestClientProfile(provider, "claude_cli");
  expect(provider).toMatchObject({ request_client_profile: "claude_cli", api_key: "fixture", credentials: [{ id: "one" }], site_health: {} });
  writeRequestClientProfile(provider, "auto");
  expect(provider).not.toHaveProperty("request_client_profile");
});

it("offers all profiles and honors the editor lock", async () => {
  const user = userEvent.setup();
  const onChange = vi.fn();
  const view = render(<RequestClientProfileSelect t={(zh) => zh} onChange={onChange} />);
  const select = screen.getByRole("combobox", { name: "上游客户端参数" });
  expect(select).toHaveValue("auto");
  expect(screen.getAllByRole("option")).toHaveLength(6);
  await user.selectOptions(select, "workbuddy");
  expect(onChange).toHaveBeenCalledWith("workbuddy");
  await user.selectOptions(select, "cpa");
  expect(onChange).toHaveBeenCalledWith("cpa");
  await user.selectOptions(select, "cpa_claude_code");
  expect(onChange).toHaveBeenCalledWith("cpa_claude_code");
  view.rerender(<RequestClientProfileSelect t={(zh) => zh} value="cpa" disabled onChange={onChange} />);
  expect(select).toBeDisabled();
});

it("serializes the selected profile when creating a provider", () => {
  const template = PROVIDER_CATALOG_TEMPLATES.find((entry) => entry.id === "custom-api-provider")!;
  const draft = { templateId: template.id, providerId: "relay", providerLabel: "Relay", vendorKey: "relay", vendorName: "Relay",
    baseUrl: "https://relay.example", supportedModels: [], credentialId: "one", accountName: "One", apiKey: "fixture",
    requestClientProfile: "workbuddy" as const };
  expect(providerDefinitionFromCatalogDraft(template, draft)).toHaveProperty("request_client_profile", "workbuddy");
});

it("edits an existing provider from its card without touching the credentials", async () => {
  const user = userEvent.setup();
  const onUpdateProviderRequestClientProfile = vi.fn();
  render(<AccountsLedgerWorkspace {...workspaceProps({
    pilotSections: [pilotSection({ requestClientProfile: "cpa" })], onUpdateProviderRequestClientProfile,
  })} />);
  await user.click(screen.getByRole("button", { name: "翻面查看 Managed OpenAI 详情" }));
  const select = screen.getByRole("combobox", { name: "上游客户端参数" });
  expect(select).toHaveValue("cpa");
  await user.selectOptions(select, "codex_cli");
  expect(onUpdateProviderRequestClientProfile).toHaveBeenCalledExactlyOnceWith("managed-provider", "codex_cli");
});
