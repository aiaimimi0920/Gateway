import "@testing-library/jest-dom/vitest";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient } from "../../api/client";
import { SiteHealthWorkspace } from "./SiteHealthWorkspace";
import { parseRouteDocument } from "./routeDocument";

const document = parseRouteDocument(
  JSON.stringify({
    providers: [
      { id: "site", base_url: "https://fixture.invalid/v1", site_health: {} },
    ],
    model_routes: [],
    aliases: {},
  }),
);
const report = {
  items: [
    {
      provider_id: "site",
      config: {},
      snapshot: {
        provider_id: "site",
        checked_at: 1000,
        binding: "binding",
        selected_template: "channel-health",
        models: { model: 98 },
        probes: [
          {
            template: "channel-health",
            endpoint: "https://fixture.invalid/api/channel/health/models",
            status: "supported",
            http_status: 200,
            model_count: 1,
            models: {},
          },
        ],
      },
    },
  ],
};
const t = (zh: string) => zh;

it("does not poll until expanded and saves independent policy fields without choosing a template", async () => {
  const request = vi.fn().mockResolvedValue(report);
  const save = vi.fn();
  const view = render(
    <SiteHealthWorkspace
      document={document}
      client={{ request } as unknown as GatewayApiClient}
      managementToken="fixture"
      locked={false}
      draftDirty={false}
      onSave={save}
      t={t}
    />,
  );
  expect(request).not.toHaveBeenCalled();
  fireEvent.click(screen.getByText("站点健康"));
  await waitFor(() => expect(request).toHaveBeenCalledTimes(1));
  expect(await screen.findByText("98.0%")).toBeInTheDocument();
  fireEvent.change(screen.getByLabelText("控制策略"), {
    target: { value: "restrict" },
  });
  expect(screen.getByRole("button", { name: "立即识别接口" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("监控分组（逗号分隔）"), {
    target: { value: "paid, trial" },
  });
  expect(screen.queryByLabelText("模型映射")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(save).toHaveBeenCalledWith(
    "site",
    expect.objectContaining({
      mode: "restrict",
      monitor_groups: ["paid", "trial"],
      model_map: {},
    }),
  );
  const signal = request.mock.calls[0][2].signal as AbortSignal;
  view.unmount();
  expect(signal.aborted).toBe(true);
});

it("cancels manual discovery and rejects late UI updates after a token change", async () => {
  let resolve!: (value: unknown) => void;
  const request = vi.fn().mockImplementation((path: string) =>
    path.endsWith("/refresh")
      ? new Promise((done) => {
          resolve = done;
        })
      : Promise.resolve(report),
  );
  const client = { request } as unknown as GatewayApiClient;
  const props = {
    document,
    client,
    managementToken: "first",
    locked: false,
    draftDirty: false,
    onSave: vi.fn(),
    t,
  };
  const view = render(<SiteHealthWorkspace {...props} />);
  fireEvent.click(screen.getByText("站点健康"));
  await screen.findByText("98.0%");
  fireEvent.click(screen.getByRole("button", { name: "立即识别接口" }));
  const call = request.mock.calls.find(([path]) => path.endsWith("/refresh"))!;
  expect(call[2].method).toBe("POST");
  view.rerender(<SiteHealthWorkspace {...props} managementToken="second" />);
  expect(call[2].signal.aborted).toBe(true);
  await act(async () => resolve({ snapshot: report.items[0].snapshot }));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  view.unmount();
});

it("pins settings to the selected provider and never falls back to another site", async () => {
  const request = vi.fn().mockResolvedValue(report);
  const save = vi.fn();
  const sites = parseRouteDocument(
    JSON.stringify({
      providers: [
        { id: "site", base_url: "https://fixture.invalid", site_health: {} },
        {
          id: "second",
          base_url: "https://second.invalid",
          site_health: { mode: "restrict" },
        },
      ],
      model_routes: [],
      aliases: {},
    }),
  );
  const props = {
    document: sites,
    client: { request } as unknown as GatewayApiClient,
    managementToken: "fixture",
    locked: false,
    draftDirty: false,
    onSave: save,
    t,
  };
  const view = render(
    <SiteHealthWorkspace {...props} providerId="second" initiallyOpen />,
  );
  expect(
    screen.queryByRole("combobox", { name: "站点" }),
  ).not.toBeInTheDocument();
  expect(screen.getByLabelText("控制策略")).toHaveValue("restrict");
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(save).toHaveBeenCalledWith(
    "second",
    expect.objectContaining({ mode: "restrict" }),
  );
  view.rerender(
    <SiteHealthWorkspace {...props} providerId="removed" initiallyOpen />,
  );
  expect(screen.getByRole("button", { name: "保存站点策略" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "立即识别接口" })).toBeDisabled();
  view.unmount();
  expect((request.mock.calls[0][2].signal as AbortSignal).aborted).toBe(true);
});
