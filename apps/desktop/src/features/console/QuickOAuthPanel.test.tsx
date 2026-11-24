import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { QuickAuthSession } from "../../api/console/quick-auth";
import { GatewayApiError } from "../../api/errors";
import { providerAccessKind } from "./providerAccessKind";
import { PROVIDER_CATALOG_TEMPLATES, providerDefinitionFromCatalogDraft } from "./providerCatalog";
import { defaultCatalogWorkflow } from "./providerCatalogWorkflow";
import { QUICK_OAUTH_TEMPLATE } from "./quickCatalog";
import { QuickOAuthPanel, type QuickOAuthProps } from "./QuickOAuthPanel";

vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));

const api = vi.hoisted(() => ({ create: vi.fn(), get: vi.fn(), act: vi.fn() }));
vi.mock("../../api/console/quick-auth", () => ({
  createQuickAuthApi: () => api,
  validQuickRedirect: (s: string) => /^http:\/\/127\.0\.0\.1:51234$/.test(s),
}));

const makeSession = (
  status: QuickAuthSession["status"],
  step: QuickAuthSession["step"] = "discovery",
): QuickAuthSession => ({
  id: "quick-sess-1",
  providerId: "quick-oauth",
  expectedRevision: "rev-1",
  status,
  step,
  authorizationUrl:
    status === "waiting_user"
      ? step === "discovery"
        ? "https://quick.aws.com/sn?qwSurface=desktop%3Anonce1%3A51234%3A1.2.4660"
        : "https://oidc.us-east-1.amazonaws.com/authorize?response_type=code&client_id=cid&state=st"
      : null,
  redirectUri: "http://127.0.0.1:51234",
  accountSummary: status === "ready" ? "alice@example.com" : null,
  credentialId: status === "succeeded" ? "quick-cred-1" : null,
  targetCredentialId: null,
  savedRevision: status === "succeeded" ? "rev-2" : null,
  errorCode: null,
});

const makeProps = (): QuickOAuthProps => ({
  client: { request: vi.fn() },
  managementToken: "mgr-token",
  secretGrant: "sec-grant",
  providerId: "quick-oauth",
  expectedRevision: "rev-1",
  operations: new Map(),
  disabled: false,
  onRequestSecretAccess: vi.fn(),
  onSaved: vi.fn(),
});

describe("quick-oauth catalog & QuickOAuthPanel", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    api.create.mockResolvedValue(makeSession("waiting_user", "discovery"));
    api.get.mockResolvedValue(makeSession("ready", "authorize"));
    api.act.mockResolvedValue(makeSession("cancelled"));
  });

  afterEach(() => vi.useRealTimers());

  it("registers quick-oauth in catalog with expected models and explicit OAuth definition", () => {
    const found = PROVIDER_CATALOG_TEMPLATES.find((t) => t.id === "quick-oauth");
    expect(found).toBeDefined();
    expect(found?.providerId).toBe("quick-oauth");
    expect(found?.supportedModels).toEqual(["smart", "fast", "balanced", "auto"]);
    expect(providerAccessKind({ providerPreset: QUICK_OAUTH_TEMPLATE.preset })).toBe("oauth");

    const definition = providerDefinitionFromCatalogDraft(QUICK_OAUTH_TEMPLATE, {
      templateId: "quick-oauth",
      providerId: "quick-oauth",
      providerLabel: "quick-oauth",
      vendorKey: "amazon-quick",
      vendorName: "Amazon Quick",
      baseUrl: "https://quick.aws.com",
      supportedModels: [...QUICK_OAUTH_TEMPLATE.supportedModels],
      workflow: defaultCatalogWorkflow(QUICK_OAUTH_TEMPLATE),
    });
    expect(definition.preset).toBe("quick-oauth");
    expect(definition.auth_mode).toBe("quick_oauth");
    expect(definition.explicit_credentials_only).toBe(true);
  });

  it("supports two-step manual callback paste and explicit credential save", async () => {
    const p = makeProps();
    api.act
      .mockResolvedValueOnce(makeSession("waiting_user", "authorize"))
      .mockResolvedValueOnce(makeSession("ready", "authorize"))
      .mockResolvedValueOnce(makeSession("succeeded", "authorize"));

    render(<QuickOAuthPanel {...p} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "通过 Amazon Quick 登录" }));
    });
    expect(screen.getByRole("link")).toHaveAttribute(
      "href",
      "https://quick.aws.com/sn?qwSurface=desktop%3Anonce1%3A51234%3A1.2.4660",
    );

    const input = screen.getByPlaceholderText("http://127.0.0.1:51234/callback?nonce=...");
    fireEvent.change(input, {
      target: { value: "http://127.0.0.1:51234/callback?nonce=nonce1&issuerUrl=https://d-1.awsapps.com/start" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "提交回调地址" }));
    });
    expect(screen.getByRole("link")).toHaveAttribute(
      "href",
      "https://oidc.us-east-1.amazonaws.com/authorize?response_type=code&client_id=cid&state=st",
    );

    fireEvent.change(screen.getByPlaceholderText("http://127.0.0.1:51234/callback?nonce=..."), {
      target: { value: "http://127.0.0.1:51234/?code=code1&state=st" },
    });
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "提交回调地址" }));
    });
    expect(screen.getByText(/alice@example\.com/)).toBeInTheDocument();
    expect(p.onSaved).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "保存到凭证池" }));
    });
    expect(p.onSaved).toHaveBeenCalledOnce();
  });

  it("unlocks sign-in when an unsaved session becomes unavailable", async () => {
    api.get.mockRejectedValue(new GatewayApiError("quick_oauth_login_unavailable", 400, "quick_oauth_login_rejected"));
    render(<QuickOAuthPanel {...makeProps()} />);
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "通过 Amazon Quick 登录" }));
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(1500);
    });
    expect(screen.getByText("授权会话已不可用，请重新授权。")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "通过 Amazon Quick 登录" })).toBeEnabled();
  });
});
