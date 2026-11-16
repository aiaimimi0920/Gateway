// Full Vertex is an explicit project/location OAuth pool, never an Express or AI Studio key pool.
import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { ChatgptAuthorizationLink } from "./ChatgptAuthorizationLink";
import { PROVIDER_CATALOG_TEMPLATES } from "./providerCatalog";
import { addCatalogProvider } from "./providerCatalogCreation";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { catalogProviderKind } from "./officialModelCatalog";
import { supportsAccountDiscovery } from "./accountDiscovery";

beforeEach(() => localStorage.setItem("gateway-ui-locale", "zh-CN"));
it("creates a typed empty Full Vertex pool with no guessed client or model", () => {
  const entry = PROVIDER_CATALOG_TEMPLATES.find(({ id }) => id === "vertex-oauth");
  expect(entry).toBeDefined();
  const baseUrl = "https://aiplatform.googleapis.com/v1/projects/project-a/locations/global/publishers/google";
  const draft = { templateId: entry!.id, providerId: "renamed", providerLabel: "renamed", vendorKey: entry!.vendorKey,
    vendorName: entry!.vendorName, baseUrl, vertexTarget: { project: "project-a", location: "global" },
    supportedModels: [], workflow: defaultCatalogWorkflow(entry!) };
  const result = addCatalogProvider({ providers: [], model_routes: [], aliases: {} }, entry!, draft);
  expect(result.providers[0]).toMatchObject({ id: "renamed", preset: "google-agent-platform-official-api",
    auth_mode: "vertex_user_oauth", base_url: baseUrl, explicit_credentials_only: true, credentials: [], supported_models: [],
    test_policy: { automaticEnabled: false, models: [] } });
  for (const key of ["api_key", "vertex_oauth", "refresh_client_id", "default_model", "headers"]) expect(result.providers[0]).not.toHaveProperty(key);
  expect(result.model_routes).toEqual([]);
  expect(catalogProviderKind(result.providers[0] as Record<string, unknown>)).toBeUndefined();
  expect(supportsAccountDiscovery(result.providers[0] as Record<string, unknown>)).toBe(false);
  expect(() => addCatalogProvider(result, entry!, { ...draft, providerId: "other", baseUrl: "https://untrusted.invalid" })).toThrow();
});
it("requires project/location and derives a non-editable target without requesting secret access", () => {
  const submit = vi.fn(); const secret = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={["vertex-oauth"]} existingCredentialIds={[]}
    locked={false} hasSecretAccess={false} onOpenChange={vi.fn()} onRequestSecretAccess={secret} onSubmit={submit} /></UiLocaleProvider>, { wrapper: NeuroProvider });
  fireEvent.click(screen.getByRole("button", { name: /^vertex-oauth/ }));
  expect(screen.getByLabelText("Project")).toHaveValue(""); expect(screen.getByLabelText("Location")).toHaveValue("");
  fireEvent.click(screen.getByRole("button", { name: "添加" })); expect(submit).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("Project"), { target: { value: "project-a" } });
  fireEvent.change(screen.getByLabelText("Location"), { target: { value: "eu" } });
  expect(screen.getByLabelText("Base URL")).toHaveAttribute("readonly");
  expect(screen.getByLabelText("Base URL")).toHaveValue("https://aiplatform.eu.rep.googleapis.com/v1/projects/project-a/locations/eu/publishers/google");
  fireEvent.click(screen.getByRole("button", { name: "添加" }));
  expect(submit).toHaveBeenCalledWith(expect.objectContaining({ templateId: "vertex-oauth", providerId: "vertex-oauth-2",
    vertexTarget: { project: "project-a", location: "eu" } })); expect(secret).not.toHaveBeenCalled();
});
it("allows only the fixed Google authorization destination for Vertex", () => {
  const view = render(<UiLocaleProvider><ChatgptAuthorizationLink kind="vertex" busy={false}
    value="https://accounts.google.com/o/oauth2/v2/auth?client_id=explicit" /></UiLocaleProvider>, { wrapper: NeuroProvider });
  expect(screen.getByRole("link")).toHaveAttribute("href", "https://accounts.google.com/o/oauth2/v2/auth?client_id=explicit");
  for (const value of ["https://auth.openai.com/oauth/authorize", "https://accounts.google.com.evil.invalid/o/oauth2/v2/auth",
    "https://accounts.google.com/o/oauth2/v2/auth#", "https://user@accounts.google.com/o/oauth2/v2/auth"]) {
    view.rerender(<UiLocaleProvider><ChatgptAuthorizationLink kind="vertex" busy={false} value={value} /></UiLocaleProvider>);
    expect(screen.queryByRole("link")).not.toBeInTheDocument();
  }
});
