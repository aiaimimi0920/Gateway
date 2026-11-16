import { NeuroProvider } from "@neuro/ui";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { RequestClientProfileSelect } from "./RequestClientProfileSelect";
import { readProviderRequestClientProfile, readRequestClientProfile, writeRequestClientProfile } from "./requestClientProfile";
import { providerDefinitionFromCatalogDraft, PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { AccountsLedgerWorkspace } from "./AccountsLedgerWorkspace";
import { pilotSection, workspaceProps } from "./AccountsLedgerWorkspace.fixtures";

async function openSettings(user: ReturnType<typeof userEvent.setup>) {
  await user.click(within(screen.getByRole("navigation", { name: "服务商目录" })).getByRole("button", { name: /Managed OpenAI/ }));
  await user.click(screen.getByRole("button", { name: "服务商设置" }));
  return screen.getByRole("dialog", { name: "服务商设置" });
}

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
  const select = screen.getByRole("combobox", { name: "客户端" });
  expect(select).toHaveValue("auto");
  expect(screen.getAllByRole("option").map((option) => [option.textContent, (option as HTMLOptionElement).value])).toEqual([
    ["自动", "auto"], ["codex", "codex_cli"], ["claude", "claude_cli"],
    ["workbuddy", "workbuddy"], ["cpa", "cpa"], ["cpa-claude", "cpa_claude_code"],
  ]);
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
  const draft = {
    templateId: template.id, providerId: "relay", providerLabel: "Relay", vendorKey: "relay", vendorName: "Relay",
    baseUrl: "https://relay.example", supportedModels: [], credentialId: "one", accountName: "One", apiKey: "fixture",
    requestClientProfile: "workbuddy" as const
  };
  expect(providerDefinitionFromCatalogDraft(template, draft)).toHaveProperty("request_client_profile", "workbuddy");
});

it("edits an existing provider from its card without touching the credentials", async () => {
  const user = userEvent.setup();
  const onUpdateProviderRequestClientProfile = vi.fn();
  render(<AccountsLedgerWorkspace {...workspaceProps({
    pilotSections: [pilotSection({ requestClientProfile: "cpa" })], onUpdateProviderRequestClientProfile,
  })} />, { wrapper: NeuroProvider });
  await openSettings(user);
  const select = screen.getByRole("combobox", { name: "客户端" });
  expect(select).toHaveValue("cpa");
  await user.selectOptions(select, "codex_cli");
  expect(onUpdateProviderRequestClientProfile).toHaveBeenCalledExactlyOnceWith("managed-provider", "codex_cli");
});

it("mounts client and site controls inside settings and removes them on close", async () => {
  const user = userEvent.setup();
  render(<AccountsLedgerWorkspace {...workspaceProps({
    onUpdateProviderRequestClientProfile: vi.fn(),
    renderProviderSettings: () => <button type="button">站点健康</button>,
  })} />, { wrapper: NeuroProvider });
  expect(screen.queryByRole("combobox", { name: "客户端" })).not.toBeInTheDocument();
  const dialog = await openSettings(user);
  const row = dialog.querySelector(".nt-provider-lifecycle__client-row") as HTMLElement;
  const client = within(row).getByRole("combobox");
  const health = within(row).getByRole("button");
  expect(row.children).toHaveLength(2);
  expect(row.children[0]).toContainElement(client);
  expect(row.children[1]).toContainElement(health);
  expect(client).toBeEnabled();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("combobox", { name: "客户端" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "站点健康" })).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "服务商设置" }));
  expect(screen.getByRole("combobox", { name: "客户端" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "站点健康" })).toBeVisible();
});

it("round-trips supported camelCase policy fields without creating duplicate aliases", () => {
  const flow = { call_adapter: "anthropic", fingerprint_adapter: "workbuddy", special_adapter: "none" };
  const provider: Record<string, unknown> = { callFlow: flow, requestClientProfile: "workbuddy" };
  expect(readProviderRequestClientProfile(provider)).toBe("workbuddy");
  writeRequestClientProfile(provider, "claude_cli");
  expect(provider).toEqual({ callFlow: { ...flow, fingerprint_adapter: "claude_cli" }, requestClientProfile: "claude_cli" });
  writeRequestClientProfile(provider, "auto");
  expect(provider).toEqual({ callFlow: { ...flow, fingerprint_adapter: "auto" } });
  const legacy: Record<string, unknown> = { requestClientProfile: "cpa" };
  expect(readProviderRequestClientProfile(legacy)).toBe("cpa");
  writeRequestClientProfile(legacy, "codex_cli");
  expect(legacy).toEqual({ requestClientProfile: "codex_cli" });
});

it("does not overwrite session-owned fingerprints or ambiguous policy aliases", () => {
  for (const provider of [
    { call_flow: { fingerprint_adapter: "session", special_adapter: "gemini_web" } },
    { callFlow: { fingerprint_adapter: "session" } },
    { call_flow: { fingerprint_adapter: "auto", special_adapter: "basispoints" } },
    { call_flow: null }, { callFlow: [] },
    { call_flow: { fingerprint_adapter: "unknown" } },
    { call_flow: {}, callFlow: {} },
    { request_client_profile: "auto", requestClientProfile: "codex_cli" },
  ]) {
    const before = structuredClone(provider);
    expect(writeRequestClientProfile(provider, "claude_cli")).toBe(false);
    expect(provider).toEqual(before);
  }
});

it("recognizes session ownership for explicit specialized adapters even with an automatic fingerprint", () => {
  for (const special_adapter of ["basispoints", "gemini_web", "gemini_canvas", "chatgpt_web", "aistudio_web"]) {
    const provider = { callFlow: { fingerprint_adapter: "auto", special_adapter } };
    expect(readProviderRequestClientProfile(provider)).toBe("session");
    const before = structuredClone(provider);
    expect(writeRequestClientProfile(provider, "claude_cli")).toBe(false);
    expect(provider).toEqual(before);
  }
});

it("shows the site session profile as a locked card control", async () => {
  const user = userEvent.setup();
  const onChange = vi.fn();
  render(<AccountsLedgerWorkspace {...workspaceProps({
    pilotSections: [pilotSection({ requestClientProfile: "session" })], onUpdateProviderRequestClientProfile: onChange,
  })} />, { wrapper: NeuroProvider });
  await openSettings(user);
  const select = screen.getByRole("combobox", { name: "客户端" });
  expect(select).toHaveValue("session");
  expect(select).toBeDisabled();
  expect(within(select).getAllByRole("option")).toHaveLength(1);
  expect(within(select).getByRole("option")).toHaveTextContent("站点会话");
  expect(onChange).not.toHaveBeenCalled();
});
