import "@testing-library/jest-dom/vitest";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { SiteHealthWorkspace } from "./SiteHealthWorkspace";
import { parseRouteDocument } from "./routeDocument";

const document = parseRouteDocument(JSON.stringify({providers:[{id:"site",base_url:"https://fixture.invalid/v1",site_health:{}}],model_routes:[],aliases:{}}));
const report = {items:[{provider_id:"site",config:{},snapshot:{provider_id:"site",checked_at:1000,binding:"binding",selected_template:"channel-health",
  models:{model:98},probes:[{template:"channel-health",endpoint:"https://fixture.invalid/api/channel/health/models",status:"supported",http_status:200,model_count:1,models:{}}]}}]};
const t = (zh: string) => zh;

it("does not poll until expanded and saves independent policy fields without choosing a template", async () => {
  const request = vi.fn().mockResolvedValue(report); const save = vi.fn();
  const view = render(<SiteHealthWorkspace document={document} client={{request} as unknown as GatewayApiClient}
    managementToken="fixture" locked={false} draftDirty={false} onSave={save} t={t} />);
  expect(request).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("站点健康"));
  await waitFor(() => expect(request).toHaveBeenCalledTimes(1));
  expect(await screen.findByText("model: 98.0%")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("控制策略"),{target:{value:"restrict"}});
  expect(screen.getByRole("button",{name:"立即识别接口"})).toBeDisabled();
  fireEvent.change(screen.getByLabelText("监控分组（逗号分隔）"),{target:{value:"paid, trial"}});
  fireEvent.change(screen.getByLabelText("模型映射"),{target:{value:"upstream=monitor"}});
  fireEvent.click(screen.getByRole("button",{name:"保存站点策略"}));
  expect(save).toHaveBeenCalledWith("site",expect.objectContaining({mode:"restrict",monitor_groups:["paid","trial"],model_map:{upstream:"monitor"}}));
  const signal = request.mock.calls[0][2].signal as AbortSignal;
  view.unmount(); expect(signal.aborted).toBe(true);
});

it("cancels manual discovery and rejects late UI updates after a token change", async () => {
  let resolve!: (value:unknown) => void;
  const request = vi.fn().mockImplementation((path:string) => path.endsWith("/refresh") ? new Promise((done) => {resolve=done;}) : Promise.resolve(report));
  const client = {request} as unknown as GatewayApiClient;
  const props = {document,client,managementToken:"first",locked:false,draftDirty:false,onSave:vi.fn(),t};
  const view = render(<SiteHealthWorkspace {...props} />);
  fireEvent.click(screen.getByText("站点健康"));
  await screen.findByText("model: 98.0%");
  fireEvent.click(screen.getByRole("button",{name:"立即识别接口"}));
  const call = request.mock.calls.find(([path]) => path.endsWith("/refresh"))!;
  expect(call[2].method).toBe("POST");
  view.rerender(<SiteHealthWorkspace {...props} managementToken="second" />);
  expect(call[2].signal.aborted).toBe(true);
  await act(async () => resolve({snapshot:report.items[0].snapshot}));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  view.unmount();
});
