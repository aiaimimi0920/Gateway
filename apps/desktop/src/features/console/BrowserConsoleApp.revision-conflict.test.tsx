import { screen, within, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import { GatewayApiError } from "../../api/errors";
import { BrowserConsoleApp } from "./BrowserConsoleApp";
import { createConsoleApi } from "./BrowserConsoleApp.api-fixture";
import { renderWithProviders, waitForConsoleReady, openWorkspace, openProviderSettings } from "./BrowserConsoleApp.render-fixture";

it.each([false, true])("handles a background revision change through the real provider save form (overlap=%s)", async overlap => {
  const api = createConsoleApi();
  let current = await api.getRouteConfig("management-secret");
  current.routeConfig.document = { providers: [{ id: "stable-id", label: "Original", adapter: "openai_compatible",
    base_url: "https://old.example/v1", explicit_credentials_only: true,
    credentials: [{ id: "account", supported_models: ["text-model"], expires_at: 10 }],
  }], model_routes: [], aliases: {} };
  current.routeConfig.secrets = [{ path: "/providers/0/credentials/0/api_key", configured: true, fingerprint: "before" }];
  vi.mocked(api.getRouteConfig).mockImplementation(async () => structuredClone(current));
  vi.mocked(api.commitRouteConfig).mockImplementation(async (_token, request) => {
    if (request.expectedRevision !== current.routeConfig.revision.id) {
      throw new GatewayApiError("Gateway console expected old revision", 409, "console_revision_conflict");
    }
    current = { routeConfig: { ...current.routeConfig, document: structuredClone(request.document),
      revision: { id: "r3-saved", sequence: 3, message: "user edit" } } };
    return { ...structuredClone(current), committed: true };
  });
  const user = userEvent.setup(); renderWithProviders(<BrowserConsoleApp consoleApi={api} />);
  await waitForConsoleReady(); await openWorkspace(user, /凭据池/i);
  const dialog = await openProviderSettings(user, /Original/);
  const form = within(within(dialog).getByRole("form", { name: "服务商信息" }));
  // Refresh on the server only, while the editor still holds its original snapshot.
  const remoteProvider = current.routeConfig.document.providers[0] as { label: string; credentials: { expires_at: number }[] };
  current.routeConfig.revision = { id: "r2-background", sequence: 2, message: "Refresh OAuth credential" };
  remoteProvider.credentials[0].expires_at = 20;
  current.routeConfig.secrets[0].fingerprint = "rotated";
  if (overlap) remoteProvider.label = "Remote name";
  await user.clear(form.getByLabelText("服务商名称")); await user.type(form.getByLabelText("服务商名称"), "New name");
  await user.click(form.getByRole("button", { name: "保存" }));
  if (overlap) {
    await waitFor(() => expect(form.getByRole("status")).toHaveTextContent("保存失败"), { timeout: 5000 });
    expect(form.getByText(/配置已在其他位置更新/)).toBeInTheDocument();
    expect(form.getByLabelText("服务商名称")).toHaveValue("New name");
    expect(api.commitRouteConfig).toHaveBeenCalledTimes(1);
    expect(remoteProvider.label).toBe("Remote name");
  } else {
    await waitFor(() => expect(form.getByRole("status")).toHaveTextContent("已保存"), { timeout: 5000 });
    expect(api.commitRouteConfig).toHaveBeenCalledTimes(2);
    expect(vi.mocked(api.commitRouteConfig).mock.calls[1][1]).toMatchObject({ expectedRevision: "r2-background",
      document: { providers: [{ label: "New name", credentials: [{ expires_at: 20 }] }] },
      secretPatches: [{ path: "/providers/0/credentials/0/api_key", operation: "keep" }] });
    await user.click(screen.getByRole("button", { name: "关闭" }));
    const reopened = await openProviderSettings(user, /New name/);
    expect(within(reopened).getByLabelText("服务商名称")).toHaveValue("New name");
  }
  expect(api.probeCredential).not.toHaveBeenCalled();
}, 15000);
