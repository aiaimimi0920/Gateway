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
      {
        id: "site",
        base_url: "https://fixture.invalid",
        site_health: {
          profiles: [
            {
              id: "plus",
              name: "Plus",
              account_group_ids: ["plus"],
              policy: {},
            },
            {
              id: "pro",
              name: "Pro",
              account_group_ids: ["pro"],
              policy: { minimum_success_rate: 99 },
            },
          ],
        },
        credentials: [{ id: "a" }, { id: "b" }],
      },
    ],
    account_groups: [
      { id: "plus", name: "Plus accounts", provider_credential_ids: ["a"] },
      { id: "pro", name: "Pro accounts", provider_credential_ids: ["b"] },
    ],
    model_routes: [],
    aliases: {},
  }),
);
const snapshot = (id: string, rate: number) => ({
  provider_id: "site",
  profile_id: id,
  checked_at: Date.now() / 1000,
  binding: id,
  selected_template: "sub2api-v2",
  models: { model: rate },
  probes: [],
});
const report = {
  items: [
    {
      provider_id: "site",
      config: {},
      snapshot: null,
      profiles: [
        {
          profile_id: "plus",
          account_group_ids: ["plus"],
          snapshot: snapshot("plus", 80),
        },
        {
          profile_id: "pro",
          account_group_ids: ["pro"],
          snapshot: snapshot("pro", 99),
        },
      ],
    },
  ],
};
const setup = (extra = {}) => {
  const request = vi.fn().mockResolvedValue(report);
  const onSave = vi.fn();
  const props = {
    document,
    client: { request } as unknown as GatewayApiClient,
    managementToken: "fixture",
    locked: false,
    draftDirty: false,
    onSave,
    t: (zh: string) => zh,
    initiallyOpen: true,
    ...extra,
  };
  const view = render(<SiteHealthWorkspace {...props} />);
  return { ...view, request, onSave, props };
};

it("keeps authorization failure and retry inside the monitoring form", async () => {
  const requestSecretAccess = vi.fn();
  const { props, rerender } = setup({
    requestSecretAccess,
    secretAccess: { busy: false, error: "Fixture authorization failed" },
  });
  await screen.findByText("80.0%");
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Fixture authorization failed",
  );
  fireEvent.click(screen.getByRole("button", { name: "授权编辑监控密钥" }));
  expect(requestSecretAccess).toHaveBeenCalledOnce();
  rerender(
    <SiteHealthWorkspace
      {...props}
      secretAccess={{ busy: true, error: null }}
    />,
  );
  expect(screen.getByRole("button", { name: "授权中…" })).toBeDisabled();
});

it("explicitly undoes a pending token operation instead of sending an empty replacement", async () => {
  const { onSave } = setup({
    hasSecretAccess: true,
    pendingSecrets: [
      {
        providerId: "site",
        profileId: "plus",
        operation: "clear",
        audience: "fixture",
      },
    ],
  });
  await screen.findByText("80.0%");
  fireEvent.click(screen.getByRole("button", { name: "撤销待提交密钥操作" }));
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(onSave.mock.calls[0][2]).toEqual([
    { profileId: "plus", operation: "keep" },
  ]);
});

it("edits and refreshes only the selected profile, keeping local bindings and remote groups separate", async () => {
  const { request, onSave } = setup();
  expect(await screen.findByText("80.0%")).toBeInTheDocument();
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Pro" }), {
    button: 0,
    ctrlKey: false,
  });
  expect(await screen.findByText("99.0%")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "立即识别接口" }));
  await waitFor(() =>
    expect(request).toHaveBeenCalledWith(
      "/v1/internal/gateway/site-health/site/profiles/pro/refresh",
      expect.anything(),
      expect.objectContaining({ method: "POST" }),
    ),
  );
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "立即识别接口" })).toBeEnabled(),
  );
  fireEvent.change(screen.getByLabelText("适配器"), {
    target: { value: "sub2api" },
  });
  fireEvent.change(screen.getByLabelText("监控分组（逗号分隔）"), {
    target: { value: "12, 18" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  const saved = onSave.mock.calls[0][1];
  expect(saved.profiles[0]).toMatchObject({
    id: "plus",
    account_group_ids: ["plus"],
  });
  expect(saved.profiles[1]).toMatchObject({
    id: "pro",
    account_group_ids: ["pro"],
    policy: { adapter: "sub2api", monitor_groups: ["12", "18"] },
  });
});

it("keeps replacement tokens out of the policy document and clears local secrets after grant loss", async () => {
  const { props, rerender, onSave } = setup({ hasSecretAccess: true });
  await screen.findByText("80.0%");
  fireEvent.change(screen.getByLabelText("适配器"), {
    target: { value: "sub2api" },
  });
  fireEvent.change(screen.getByLabelText("监控认证"), {
    target: { value: "bearer" },
  });
  fireEvent.change(screen.getByLabelText("监控密钥操作"), {
    target: { value: "replace" },
  });
  fireEvent.change(screen.getByLabelText("监控访问密钥"), {
    target: { value: "fixture-monitor-token" },
  });
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Pro" }), {
    button: 0,
    ctrlKey: false,
  });
  expect(screen.queryByLabelText("监控访问密钥")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(JSON.stringify(onSave.mock.calls[0][1])).not.toContain(
    "fixture-monitor-token",
  );
  expect(onSave.mock.calls[0][2]).toEqual([
    { profileId: "plus", operation: "replace", value: "fixture-monitor-token" },
  ]);
  rerender(<SiteHealthWorkspace {...props} hasSecretAccess={false} />);
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Plus" }), {
    button: 0,
    ctrlKey: false,
  });
  expect(screen.queryByLabelText("监控访问密钥")).not.toBeInTheDocument();
});

it("aborts an old manual refresh on profile change and does not mix late profile results", async () => {
  let done!: (result: unknown) => void;
  const request = vi.fn().mockImplementation((path: string) =>
    path.endsWith("/refresh")
      ? new Promise((resolve) => {
          done = resolve;
        })
      : Promise.resolve(report),
  );
  const { unmount } = setup({
    client: { request } as unknown as GatewayApiClient,
  });
  await screen.findByText("80.0%");
  fireEvent.click(screen.getByRole("button", { name: "立即识别接口" }));
  const signal = request.mock.calls.find(([path]) =>
    path.endsWith("/refresh"),
  )![2].signal;
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Pro" }), {
    button: 0,
    ctrlKey: false,
  });
  expect(signal.aborted).toBe(true);
  await act(async () => done({ snapshot: snapshot("plus", 0) }));
  expect(screen.getByText("99.0%")).toBeInTheDocument();
  unmount();
});

it("requires confirmation to remove a plan and allows one result to be shared across all groups", async () => {
  const { onSave } = setup();
  await screen.findByText("80.0%");
  fireEvent.click(screen.getByRole("button", { name: "删除当前方案" }));
  expect(screen.getAllByRole("tab", { hidden: true })).toHaveLength(2);
  fireEvent.click(screen.getByRole("button", { name: "取消" }));
  fireEvent.click(screen.getByRole("button", { name: "删除当前方案" }));
  fireEvent.click(screen.getByRole("button", { name: "确认删除方案" }));
  expect(screen.getByRole("tab", { name: "Pro" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  fireEvent.click(screen.getByLabelText("全部分组共享一次检测"));
  expect(screen.getByRole("button", { name: "新增检测方案" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(onSave.mock.calls[0][1].profiles).toEqual([
    expect.objectContaining({ id: "pro", account_group_ids: [] }),
  ]);
});

it("creates stable independent plan identities and prevents binding a group twice", async () => {
  const { onSave } = setup();
  await screen.findByText("80.0%");
  fireEvent.click(screen.getByRole("button", { name: "新增检测方案" }));
  const newId = screen
    .getByRole("tab", { selected: true })
    .getAttribute("data-profile-id");
  expect(newId).toMatch(/^health-[a-f0-9-]+$/);
  expect(screen.getByLabelText(/Plus accounts/)).toBeDisabled();
  expect(screen.getByLabelText(/Pro accounts/)).toBeDisabled();
  fireEvent.change(screen.getByLabelText("方案名称"), {
    target: { value: "Default monitor" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(onSave.mock.calls[0][1].profiles[2]).toMatchObject({
    id: newId,
    name: "Default monitor",
    account_group_ids: ["default"],
    policy: { auth: { kind: "none" } },
  });
});

it("fences unsubmitted token edits when the target changes and never sends a manual probe", async () => {
  const { onSave, request } = setup({ hasSecretAccess: true });
  await screen.findByText("80.0%");
  fireEvent.change(screen.getByLabelText("适配器"), {
    target: { value: "sub2api" },
  });
  fireEvent.change(screen.getByLabelText("监控认证"), {
    target: { value: "bearer" },
  });
  fireEvent.change(screen.getByLabelText("监控密钥操作"), {
    target: { value: "replace" },
  });
  fireEvent.change(screen.getByLabelText("监控访问密钥"), {
    target: { value: "fixture-token" },
  });
  fireEvent.change(screen.getByLabelText("监控站地址（留空使用本站）"), {
    target: { value: "https://changed.invalid" },
  });
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(screen.getByRole("alert")).toHaveTextContent("监控目标或认证已改变");
  expect(onSave).not.toHaveBeenCalled();
  expect(screen.getByRole("button", { name: "立即识别接口" })).toBeDisabled();
  expect(request).toHaveBeenCalledTimes(1);
});

it("pauses only the selected plan, preserves its policy and restores it on resume", async () => {
  const { onSave } = setup({ hasSecretAccess: true });
  await screen.findByText("80.0%");
  fireEvent.click(screen.getByRole("switch", { name: "启用此方案" }));
  expect(screen.getByLabelText("适配器")).toBeDisabled();
  expect(screen.getByRole("button", { name: "立即识别接口" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(onSave.mock.calls[0][1].profiles[0]).toMatchObject({
    id: "plus",
    policy: { enabled: false },
  });
  expect(onSave.mock.calls[0][1].profiles[1]).toMatchObject({
    id: "pro",
    policy: { enabled: true, minimum_success_rate: 99 },
  });
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Pro" }), { button: 0 });
  expect(screen.getByRole("switch", { name: "启用此方案" })).toBeChecked();
  fireEvent.mouseDown(screen.getByRole("tab", { name: "Plus" }), { button: 0 });
  expect(screen.getByRole("switch", { name: "启用此方案" })).not.toBeChecked();
  fireEvent.click(screen.getByRole("switch", { name: "启用此方案" }));
  fireEvent.click(screen.getByRole("button", { name: "保存站点策略" }));
  expect(onSave.mock.calls[1][1].profiles[0]).toMatchObject({
    id: "plus",
    policy: { enabled: true, minimum_success_rate: 95 },
  });
});
