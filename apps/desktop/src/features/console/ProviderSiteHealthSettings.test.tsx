import { fireEvent, render as renderBase, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { ProviderSiteHealthSettings } from "./ProviderSiteHealthSettings";
import { parseRouteDocument } from "./routeDocument";
import { NeuroProvider } from "@neuro/ui";
const render = (ui: Parameters<typeof renderBase>[0]) => renderBase(ui, { wrapper: NeuroProvider });

it("cancels provider monitoring on Escape, restores focus, and resets unsaved settings when reopened", async () => {
  const document = parseRouteDocument(JSON.stringify({
    providers: [
      { id: "site", label: "Fixture", base_url: "https://fixture.invalid", site_health: {} },
    ], model_routes: [], aliases: {}
  }));
  const request = vi.fn().mockImplementation((path: string) => path.endsWith("/refresh")
    ? new Promise(() => { }) : Promise.resolve({ items: [] }));
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

it("cancels only the nested confirmation on Escape and permits closing after permission is revoked", async () => {
  const user = userEvent.setup(), onSave = vi.fn();
  const document = parseRouteDocument(JSON.stringify({
    providers: [{
      id: "site", site_health: {
        profiles: [
          { id: "one", name: "Plan one", account_group_ids: [], policy: {} },
        ]
      }
    }], model_routes: [], aliases: {}
  }));
  const request = vi.fn().mockResolvedValue({ items: [] });
  const props = {
    document, client: { request } as unknown as GatewayApiClient, providerId: "site", providerLabel: "Fixture",
    managementToken: "fixture", locked: false, draftDirty: false, onSave, t: (zh: string) => zh
  };
  const view = render(<ProviderSiteHealthSettings {...props} />);
  const trigger = screen.getByRole("button", { name: "站点健康" });
  await user.click(trigger);
  const remove = screen.getByRole("button", { name: "删除当前方案" });
  await user.click(remove);
  expect(screen.getByRole("button", { name: "取消" })).toHaveFocus();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(screen.getByRole("dialog")).toBeInTheDocument();
  expect(remove).toHaveFocus();
  await user.click(remove);
  view.rerender(<ProviderSiteHealthSettings {...props} locked />);
  expect(screen.getByRole("button", { name: "确认删除方案" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "确认删除方案" }));
  expect(onSave).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "取消" }));
  expect(screen.getByRole("tab", { name: "Plan one" })).toHaveFocus();
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(trigger).toHaveFocus();
  expect(onSave).not.toHaveBeenCalled();
});

it("keeps the modal frame and fields on pause, and returns to the legacy tab after deleting the final plan", async () => {
  const user = userEvent.setup(), onSave = vi.fn();
  const document = parseRouteDocument(JSON.stringify({
    providers: [{
      id: "site", site_health: {
        profiles: [
          { id: "one", name: "Plan one", account_group_ids: [], policy: { minimum_success_rate: 88 } },
        ]
      }
    }], model_routes: [], aliases: {}
  }));
  render(<ProviderSiteHealthSettings document={document} client={{ request: vi.fn().mockResolvedValue({ items: [] }) } as unknown as GatewayApiClient}
    providerId="site" providerLabel="Fixture" managementToken="fixture" locked={false} draftDirty={false} onSave={onSave} t={(zh) => zh} />);
  await user.click(screen.getByRole("button", { name: "站点健康" }));
  const dialog = screen.getByRole("dialog"), adapter = screen.getByLabelText("适配器"), toggle = screen.getByRole("switch");
  await user.click(toggle);
  expect(screen.getByRole("dialog")).toBe(dialog);
  expect(screen.getByLabelText("适配器")).toBe(adapter);
  expect(adapter).toBeDisabled();
  expect(toggle).toHaveFocus();
  await user.click(toggle);
  expect(screen.getByLabelText("最低成功率 %")).toHaveValue(88);
  await user.click(screen.getByRole("button", { name: "删除当前方案" }));
  await user.click(screen.getByRole("button", { name: "确认删除方案" }));
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(screen.getByRole("tab", { name: "默认方案" })).toHaveFocus();
  expect(onSave).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(onSave).toHaveBeenCalledWith("site", expect.objectContaining({ profiles: [], enabled: false }));
});
