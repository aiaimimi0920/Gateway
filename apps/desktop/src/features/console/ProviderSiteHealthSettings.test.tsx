import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { ProviderSiteHealthSettings } from "./ProviderSiteHealthSettings";
import { parseRouteDocument } from "./routeDocument";

it("cancels provider monitoring on Escape, restores focus, and resets unsaved settings when reopened", async () => {
  const document = parseRouteDocument(JSON.stringify({ providers: [
    { id: "site", label: "Fixture", base_url: "https://fixture.invalid", site_health: {} },
  ], model_routes: [], aliases: {} }));
  const request = vi.fn().mockImplementation((path: string) => path.endsWith("/refresh")
    ? new Promise(() => {}) : Promise.resolve({ items: [] }));
  const user = userEvent.setup();
  render(<ProviderSiteHealthSettings document={document} client={{ request } as unknown as GatewayApiClient}
    providerId="site" providerLabel="Fixture" managementToken="fixture" locked={false}
    draftDirty={false} onSave={vi.fn()} t={(zh) => zh} />);
  const trigger = screen.getByRole("button", { name: "站点健康" });
  expect(request).not.toHaveBeenCalled();
  await user.click(trigger);
  const dialog = await screen.findByRole("dialog", { name: "Fixture · 站点健康" });
  await user.click(within(dialog).getByRole("button", { name: "立即识别接口" }));
  const signals = request.mock.calls.map((call) => call[2].signal as AbortSignal);
  expect(signals).toHaveLength(2);
  expect(signals.every((signal) => !signal.aborted)).toBe(true);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(signals.every((signal) => signal.aborted)).toBe(true);
  expect(trigger).toHaveFocus();
  await user.click(trigger);
  await user.selectOptions(screen.getByLabelText("控制策略"), "restrict");
  await user.click(screen.getByRole("button", { name: "关闭" }));
  expect(trigger).toHaveFocus();
  await user.click(trigger);
  expect(screen.getByLabelText("控制策略")).toHaveValue("advisory");
  expect(screen.getByRole("button", { name: "立即识别接口" })).toBeEnabled();
});
