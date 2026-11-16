import { NeuroProvider } from "@neuro/ui";
import { fireEvent, render as renderUi, screen } from "@testing-library/react";
import type { ReactElement } from "react";
import { expect, it, vi } from "vitest";
import type { ConsoleRouteConfigResponse, ConsoleRouteDocument } from "../../api/contracts";
import { QwenProviderEndpointSettings } from "./QwenProviderEndpointSettings";
const base = "https://workspace-a.cn-beijing.maas.aliyuncs.com/compatible-mode/v1";
const render = (element: ReactElement) => renderUi(element, { wrapper: NeuroProvider });
const document = (): ConsoleRouteDocument => ({
  providers: [{
    id: "ordinary", preset: "qwen-official-api", base_url: base,
    explicit_credentials_only: true, credentials: [{ id: "a" }]
  }], model_routes: [], aliases: {}
});
function props() {
  return {
    providerId: "ordinary", providerLabel: "Qwen", document: document(), activeConfig: null as ConsoleRouteConfigResponse | null,
    locked: false, onApply: vi.fn().mockReturnValue(true), t: (zh: string) => zh
  };
}
const open = () => fireEvent.click(screen.getByRole("button", { name: "Qwen 默认目标" }));
const edit = () => fireEvent.change(screen.getByLabelText("Qwen 工作空间"), { target: { value: "workspace-new" } });
it("revalidates the source snapshot and preserves local input on unrelated rerenders", () => {
  const p = props(); const view = render(<QwenProviderEndpointSettings {...p} />); open(); edit();
  view.rerender(<QwenProviderEndpointSettings {...p} document={structuredClone(p.document)} />);
  expect(screen.getByLabelText("Qwen 工作空间")).toHaveValue("workspace-new");
  const changed = structuredClone(p.document); (changed.providers[0] as Record<string, unknown>).label = "new";
  view.rerender(<QwenProviderEndpointSettings {...p} document={changed} />);
  expect(screen.getByRole("alert")).toHaveTextContent("草稿已变化");
  expect(screen.getByRole("button", { name: "应用到草稿" })).toBeDisabled(); expect(p.onApply).not.toHaveBeenCalled();
});
it("revoking mutation permission closes and discards the editor", () => {
  const p = props(); const view = render(<QwenProviderEndpointSettings {...p} />); open(); edit();
  view.rerender(<QwenProviderEndpointSettings {...p} locked />);
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  view.rerender(<QwenProviderEndpointSettings {...p} />); open();
  expect(screen.getByLabelText("Qwen 工作空间")).toHaveValue("workspace-a"); expect(p.onApply).not.toHaveBeenCalled();
});
it("requires consent again after changing the proposed audience", () => {
  const p = props(); render(<QwenProviderEndpointSettings {...p} />); open(); edit();
  fireEvent.click(screen.getByRole("checkbox", { name: /固定原目标/ }));
  expect(screen.getByRole("button", { name: "应用到草稿" })).toBeEnabled();
  fireEvent.change(screen.getByLabelText("Qwen 地域"), { target: { value: "eu-central-1" } });
  expect(screen.getByRole("checkbox")).not.toBeChecked();
  expect(screen.getByRole("button", { name: "应用到草稿" })).toBeDisabled();
});
it.each([{ credentials: [] }, { credentials: [{ id: "override", base_url: base }] }])("allows the first default for an empty parent with only explicit overrides", ({ credentials }) => {
  const p = props(); Object.assign(p.document.providers[0] as Record<string, unknown>, { base_url: "", credentials });
  render(<QwenProviderEndpointSettings {...p} />); open();
  fireEvent.change(screen.getByLabelText("Qwen 地域"), { target: { value: "cn-hongkong" } }); edit();
  fireEvent.click(screen.getByRole("button", { name: "应用到草稿" }));
  expect(p.onApply).toHaveBeenCalledWith("ordinary", JSON.stringify(p.document.providers[0]), { workspaceId: "workspace-new", region: "cn-hongkong" }, false);
});
it("blocks legacy parent material, but not an unrelated storage password", () => {
  const p = props(); const config: ConsoleRouteConfigResponse = {
    routeConfig: {
      document: p.document, revision: { id: "r1", sequence: 1 },
      source: "local", diagnostics: { diagnostics: [] }, requiresRepair: false, mutationSupported: true,
      secrets: [{ path: "/providers/0/api_key", configured: true }]
    }
  };
  const view = render(<QwenProviderEndpointSettings {...p} activeConfig={config} />);
  expect(screen.getByRole("button", { name: "Qwen 默认目标" })).toBeDisabled();
  config.routeConfig.secrets = [{ path: "/providers/0/credential_storage_password", configured: true }];
  view.rerender(<QwenProviderEndpointSettings {...p} activeConfig={config} />);
  expect(screen.getByRole("button", { name: "Qwen 默认目标" })).toBeEnabled();
});
it("does not expose ordinary-target editing for Coding Plan", () => {
  const p = props(); (p.document.providers[0] as Record<string, unknown>).preset = "qwen-coding-plan-openai";
  render(<QwenProviderEndpointSettings {...p} />); expect(screen.queryByRole("button")).not.toBeInTheDocument();
});
