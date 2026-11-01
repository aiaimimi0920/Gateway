import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { ChatgptOAuthPanel } from "./ChatgptOAuthPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
it("uses the independent Antigravity import API and official OAuth URL, never auto-generates", async () => {
  const request = vi.fn().mockResolvedValue({ session: { id: "agy-login", status: "waiting_user", message: "waiting",
    authorizationUrl: "https://accounts.google.com/o/oauth2/v2/auth?state=synthetic", credentialId: null } });
  const client = { request } as GatewayApiClient;
  const saved = vi.fn();
  const view = render(<ChatgptOAuthPanel kind="antigravity" client={client} managementToken="operator" secretGrant="grant"
    providerId="custom-agy" disabled={false} onRequestSecretAccess={vi.fn()} onSaved={saved} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Antigravity 登录" })); });
  expect(request.mock.calls[0][0]).toBe("/v1/internal/gateway/console/antigravity-auth-sessions");
  expect(request.mock.calls[0][2]).toMatchObject({ method: "POST", secretGrant: "grant", body: { providerId: "custom-agy" } });
  expect(request.mock.calls[0][2].body).not.toHaveProperty("group");
  expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  expect(screen.getByRole("link", { name: "打开授权页面" })).toHaveAttribute("href", "https://accounts.google.com/o/oauth2/v2/auth?state=synthetic");
  expect(saved).not.toHaveBeenCalled();
  view.unmount();
  expect(request.mock.calls[1][0]).toBe("/v1/internal/gateway/console/antigravity-auth-sessions/agy-login/cancel");
});

it.each([
  "https://accounts.google.com.evil.invalid/o/oauth2/v2/auth",
  "https://accounts.google.com/o/oauth2/v2/auth#fragment",
  "https://accounts.google.com:444/o/oauth2/v2/auth",
  "https://accounts.google.com/o/oauth2/token",
  "https://auth.openai.com/oauth/authorize",
])("rejects cross-provider or unsafe authorization URL %s", async (authorizationUrl) => {
  const client = { request: vi.fn().mockResolvedValue({ session: { id: "login", status: "waiting_user", message: "waiting", authorizationUrl, credentialId: null } }) } as GatewayApiClient;
  render(<ChatgptOAuthPanel kind="antigravity" client={client} managementToken="operator" secretGrant="grant"
    providerId="agy" disabled={false} onRequestSecretAccess={vi.fn()} onSaved={vi.fn()} />);
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: "通过 Antigravity 登录" })); });
  expect(screen.queryByRole("link")).not.toBeInTheDocument();
  expect(screen.getByRole("alert")).toHaveTextContent("授权链接无效");
});
