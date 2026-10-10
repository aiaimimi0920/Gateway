import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, renderHook, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { ProviderCatalogDialog } from "./ProviderCatalogDialog";
import { CredentialDialog } from "./CredentialDialog";
import { accountDiscoverySchema, applyAccountDiscovery, type AccountDiscovery } from "./accountDiscovery";
import { useDiscoverySubmission } from "./useDiscoverySubmission";
import { parseRouteDocument } from "./routeDocument";

const discovery: AccountDiscovery = {
  source_url: "https://fixture.test", api_base: "https://fixture.test/v1",
  protocol: "chat_completions", models: ["model-a", "model-b"], verified_models: ["model-a"],
  checked_at: "2026-10-08T00:00:00Z", binding: "fixture-binding",
};
beforeEach(() => window.localStorage.setItem("gateway-ui-locale", "zh-CN"));

it("retains the bounded expanded alias attempt history", () => {
  const attempts = Array.from({ length: 24 }, () => ({ endpoint: "https://fixture.test/gemini/v1/models/a:generateContent", model: "a", status: "invalid_response", http_status: 200 }));
  const body = { ...discovery, probes: [{ protocol: "gemini_generate_content", routing_ready: false, status: "unconfirmed", attempts }] };
  expect(accountDiscoverySchema.parse(body).probes?.[0].attempts).toHaveLength(24);
  expect(accountDiscoverySchema.safeParse({ ...body, probes: [{ ...body.probes[0], attempts: [...attempts, attempts[0]] }] }).success).toBe(false);
});

it("preserves extended inventory outcomes without claiming routing readiness", () => {
  const names = ["chat_completions", "responses", "messages", "gemini_generate_content", "gemini_interactions",
    "ollama_chat", "ollama_generate", "cohere_chat", "bedrock_converse", "completions", "dashscope_text", "dashscope_multimodal"];
  const probes = names.map((protocol) => ({ protocol, routing_ready: false, status: "unconfirmed",
    attempts: [{ endpoint: "https://fixture.test/v1/models/a:generateContent", model: "a", status: "authentication_required", http_status: 403 }] }));
  const parsed = accountDiscoverySchema.parse({ ...discovery, probes });
  expect(parsed.probes).toEqual(probes);
  expect(accountDiscoverySchema.parse({ ...discovery, protocol: "gemini_generate_content" }).protocol).toBe("gemini_generate_content");
});

it("retains all protocol evidence through schema parsing and account refresh", () => {
  const protocols = ["chat_completions", "responses", "messages"].map((protocol) => ({
    protocol, api_base: discovery.api_base, verified_models: ["model-a"], failed_models: ["model-b"],
  }));
  const parsed = accountDiscoverySchema.parse({ ...discovery, protocols });
  const document = parseRouteDocument(JSON.stringify({ providers: [{ id: "old-pool-id", credentials: [{ id: "old-account-id" }] }], model_routes: [], aliases: {} }));
  const next = applyAccountDiscovery(document, "old-pool-id", "old-account-id", parsed);
  expect(next.providers[0]).toMatchObject({ id: "old-pool-id", credentials: [{ id: "old-account-id", discovery: { protocols } }] });
  expect(accountDiscoverySchema.parse(discovery).protocols).toBeUndefined();
});

function catalog(onDiscover = vi.fn().mockResolvedValue(discovery)) {
  const onSubmit = vi.fn(); const onOpenChange = vi.fn();
  render(<UiLocaleProvider><ProviderCatalogDialog open existingProviderIds={[]} existingCredentialIds={[]}
    locked={false} hasSecretAccess onOpenChange={onOpenChange} onRequestSecretAccess={vi.fn()}
    onSubmit={onSubmit} onDiscover={onDiscover} /></UiLocaleProvider>);
  fireEvent.click(screen.getByRole("button", { name: /自定义 API 服务商/i }));
  fireEvent.change(screen.getByLabelText("Base URL"), { target: { value: discovery.source_url } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  return { onSubmit, onOpenChange, onDiscover };
}

it("submits the provider immediately using only root URL and key", async () => {
  const props = catalog();
  expect(screen.queryByLabelText("支持模型与聚合路由")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "创建服务商与首个账号" }));
  await waitFor(() => expect(props.onSubmit).toHaveBeenCalledWith(expect.objectContaining({
    baseUrl: discovery.source_url, supportedModels: [], apiKey: "fixture-key",
  })));
  expect(props.onDiscover).not.toHaveBeenCalled();
  expect(props.onOpenChange).toHaveBeenCalledWith(false);
});

it("does not wait for a slow discovery request before closing creation", () => {
  const props = catalog(vi.fn(() => new Promise(() => {})));
  fireEvent.click(screen.getByRole("button", { name: "创建服务商与首个账号" }));
  expect(props.onSubmit).toHaveBeenCalledTimes(1);
  expect(props.onOpenChange).toHaveBeenCalledWith(false);
  expect(props.onDiscover).not.toHaveBeenCalled();
});

it("cancels a closed dialog and discards a late discovery result", async () => {
  let resolve!: (value: AccountDiscovery) => void;
  const discover = vi.fn(() => new Promise<AccountDiscovery>((done) => { resolve = done; }));
  const done = vi.fn();
  const hook = renderHook(({ open }) => useDiscoverySubmission(open, discover), { initialProps: { open: true } });
  let pending!: Promise<void>;
  act(() => { pending = hook.result.current.run({ baseUrl: discovery.source_url, apiKey: "fixture" }, done); });
  hook.rerender({ open: false });
  await act(async () => { resolve(discovery); await pending; });
  expect(done).not.toHaveBeenCalled();
});

it("adds a key to an existing generic pool using the provider root", async () => {
  const onSubmit = vi.fn(); const onDiscover = vi.fn().mockResolvedValue(discovery);
  render(<UiLocaleProvider><CredentialDialog open mode="add" providerOptions={[{ id: "p", label: "P" }]}
    existingCredentialIds={[]} locked={false} hasSecretAccess onOpenChange={vi.fn()}
    onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} onDiscover={onDiscover}
    discoveryProvider={() => ({ baseUrl: discovery.source_url })} /></UiLocaleProvider>);
  fireEvent.change(screen.getByLabelText("账号 ID"), { target: { value: "account-a" } });
  fireEvent.change(screen.getByLabelText("API Key"), { target: { value: "fixture-key" } });
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(expect.objectContaining({ apiKeyValue: "fixture-key", credentialId: "account-a" })));
  expect(onDiscover).not.toHaveBeenCalled();
});

it("refreshes one account without dropping inherited models or route policies", () => {
  const original = parseRouteDocument(JSON.stringify({ providers: [{ id: "p", supported_models: ["manual"],
    credentials: [{ id: "a", supported_models: ["old"], discovery_job: { id: "old-job", status: "pending" } }, { id: "inherited" }] }],
    model_routes: [{ pattern: "model-a", provider_ids: ["other"], enabled: false, priority: 99 }], aliases: {} }));
  const next = applyAccountDiscovery(original, "p", "a", discovery);
  const provider = next.providers[0] as Record<string, unknown>;
  expect(provider.supported_models).toEqual(["manual", "model-a", "model-b"]);
  expect(next.model_routes[0]).toMatchObject({ enabled: false, priority: 99 });
  expect(JSON.stringify(original)).not.toContain("fixture-binding");
  expect(JSON.stringify(next)).not.toContain("fixture-key");
  expect((provider.credentials as Record<string, unknown>[])[0].discovery_job).toBeUndefined();
});
