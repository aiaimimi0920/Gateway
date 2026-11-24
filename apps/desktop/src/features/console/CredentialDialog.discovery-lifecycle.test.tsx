import { useState } from "react";
import { NeuroProvider } from "@neuro/ui";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { createGatewayApiClient } from "../../api/client";
import { createBrowserHost } from "../../platform/browserHost";
import { UiLocaleProvider } from "../../i18n/UiLocaleProvider";
import { useAutomaticSecretAccess } from "../../session/useAutomaticSecretAccess";
import { createApi } from "../../session/managementSessionTestFixtures";
import type { SecretGrant } from "../../api/contracts";
import { CredentialDialog, type CredentialDialogProps } from "./CredentialDialog";
import { useAccountDiscovery } from "./useAccountDiscovery";

const discovery = { source_url: "https://fixture.test", api_base: "https://fixture.test/v1",
  protocol: "chat_completions" as const, models: ["new"], verified_models: ["new"], checked_at: "now", binding: "fixture" };
const reply = { revision: "r1", discovery };
afterEach(() => vi.useRealTimers());

function transport() {
  let resolve!: (response: Response) => void;
  let signal!: AbortSignal;
  const fetchImplementation = vi.fn((_url: string | URL | Request, init?: RequestInit) => new Promise<Response>((done, reject) => {
    resolve = done; signal = init!.signal!;
    signal.addEventListener("abort", () => reject(signal.reason), { once: true });
  }));
  const client = createGatewayApiClient({ host: createBrowserHost(), fetchImplementation });
  return { client, fetchImplementation, signal: () => signal, finish: (body = reply) => resolve(Response.json(body)) };
}

function options(client: ReturnType<typeof transport>["client"]) {
  return { client, managementToken: "manager", secretGrant: "grant", editorText: "{}", revision: "r1", draftDirty: false,
    replaceEditorDocument: vi.fn(), requestSecretAccess: vi.fn(), setError: vi.fn() };
}

it("keeps the refresh dialog alive through automatic secret-grant renewal and stages its result only once", async () => {
  vi.useFakeTimers(); localStorage.setItem("gateway-ui-locale", "zh-CN");
  const peer = transport(); const api = createApi(); const onSubmit = vi.fn();
  const grant = (id: string): SecretGrant => ({ grant: id, expiresAt: new Date(Date.now() + 300_000).toISOString() });
  vi.mocked(api.confirmSecretAccess).mockImplementation(async () => grant("renewed"));
  const initialValue: CredentialDialogProps["initialValue"] = { providerId: "pool", credentialId: "account", accountName: "Account",
    enabled: true, baseUrl: "", supportedModelsText: "old", apiKeyOperation: "keep", apiKeyValue: "" };
  const clearError = vi.fn(); const authenticationFailure = vi.fn();
  function Owner() {
    const [currentGrant, setGrant] = useState<SecretGrant | null>(() => grant("grant"));
    useAutomaticSecretAccess({ api, token: "manager", enabled: true, grant: currentGrant,
      onGrant: setGrant, onError: clearError, onAuthenticationFailure: authenticationFailure });
    const account = useAccountDiscovery({ ...options(peer.client), secretGrant: currentGrant?.grant ?? null });
    return <CredentialDialog open mode="edit" providerOptions={[{ id: "pool", label: "Pool" }]} existingCredentialIds={["account"]}
      initialValue={initialValue} locked={false} hasSecretAccess={!!currentGrant} onOpenChange={vi.fn()}
      onRequestSecretAccess={vi.fn()} onSubmit={onSubmit} discoveryProvider={() => ({ baseUrl: discovery.source_url })}
      onDiscover={account.discoverAccount} />;
  }
  render(<NeuroProvider><UiLocaleProvider><Owner /></UiLocaleProvider></NeuroProvider>);
  fireEvent.click(screen.getByRole("button", { name: "刷新协议" }));
  expect(peer.fetchImplementation).toHaveBeenCalledOnce();
  await act(async () => { await vi.advanceTimersByTimeAsync(270_000); });
  expect(api.confirmSecretAccess).toHaveBeenCalledExactlyOnceWith("manager", "manager");
  expect(peer.signal().aborted).toBe(false);
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.getByRole("button", { name: "刷新协议" })).toBeDisabled();
  await act(async () => { peer.finish(); });
  expect(screen.getByText("Chat Completions")).toBeInTheDocument();
  expect(onSubmit).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "保存" }));
  expect(onSubmit).toHaveBeenCalledExactlyOnceWith(expect.objectContaining({ discovery, apiKeyOperation: "keep" }));
  expect(peer.fetchImplementation).toHaveBeenCalledOnce();
});

it.each(["secretGrant", "managementToken", "client"] as const)("cancels discovery with an actionable error when %s is revoked or changed", async (field) => {
  const peer = transport(); const opts = options(peer.client);
  const hook = renderHook((props) => useAccountDiscovery(props), { initialProps: opts });
  let pending!: Promise<unknown>;
  act(() => { pending = hook.result.current.discoverAccount({ credentialId: "account" }); });
  const rejected = expect(pending).rejects.toThrow("配置或登录状态已变化");
  hook.rerender({ ...opts, [field]: field === "secretGrant" ? null : field === "managementToken" ? "other" : transport().client });
  await act(async () => { await rejected; });
  expect(peer.signal().aborted).toBe(true);
  expect(hook.result.current.discoveryBusy).toBe(false);
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
});

it.each(["revision", "editorText"] as const)("discards late discovery after %s changes even if the request succeeds", async (field) => {
  const peer = transport(); const opts = options(peer.client);
  const hook = renderHook((props) => useAccountDiscovery(props), { initialProps: opts });
  let pending!: Promise<unknown>;
  act(() => { pending = hook.result.current.discoverAccount({ credentialId: "account" }); });
  const rejected = expect(pending).rejects.toThrow("配置或登录状态已变化");
  hook.rerender({ ...opts, [field]: "changed" });
  await act(async () => { peer.finish(); await rejected; });
  expect(opts.replaceEditorDocument).not.toHaveBeenCalled();
});

it("preserves caller cancellation and does not replay a cancelled protocol probe", async () => {
  const peer = transport(); const opts = options(peer.client); const abort = new AbortController();
  const hook = renderHook(() => useAccountDiscovery(opts));
  let pending!: Promise<unknown>;
  act(() => { pending = hook.result.current.discoverAccount({ credentialId: "account" }, abort.signal); });
  const reason = new Error("Dialog closed"); const rejected = expect(pending).rejects.toBe(reason);
  await act(async () => { abort.abort(reason); await rejected; });
  expect(peer.fetchImplementation).toHaveBeenCalledOnce();
  expect(hook.result.current.discoveryBusy).toBe(false);
});
