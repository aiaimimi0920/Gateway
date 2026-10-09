import {
  act,
  fireEvent,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ConsoleAccessKey } from "../../api/contracts";
import { consoleAccessCatalogSchema } from "../../api/schemas/access";
import { accessKeyStatus, newAccessKeyDraft } from "./accessKeyPresentation";
import {
  deferred,
  renderWithProviders,
} from "./BrowserConsoleApp.render-fixture";
import {
  fixture,
  Harness,
  key,
  openCreate,
} from "./AccessKeysSection.fixtures";

describe("API Key manager", () => {
  beforeEach(() => window.localStorage.clear());

  it("requires a group, supports multiple groups and disables unavailable groups", async () => {
    const { api } = fixture();
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("暂无 API Key");
    await user.click(screen.getByRole("button", { name: "新建 API Key" }));
    const dialog = screen.getByRole("dialog");
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "multi-group",
    );
    const create = within(dialog).getByRole("button", { name: "创建 API Key" });
    expect(create).toBeDisabled();
    expect(
      within(dialog).getByRole("checkbox", { name: /停用组/ }),
    ).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox", { name: /基础组/ }));
    await user.click(within(dialog).getByRole("checkbox", { name: /高级组/ }));
    await user.click(create);
    await screen.findByRole("dialog", { name: "API Key 已生成" });
    expect(api.createAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      expect.objectContaining({
        metadata: { accountGroupIds: ["group-a", "group-b"] },
      }),
    );
    await user.click(screen.getByRole("button", { name: "完成" }));
    expect(screen.getByText("基础组 · 高级组")).toBeInTheDocument();
  });

  it("edits a legacy key without losing other restrictions and restores focus", async () => {
    const { api } = fixture([
      key("old", { metadata: { models: ["model-a"], scope: ["relay"] } }),
    ]);
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("未绑定（旧密钥）");
    const trigger = screen.getByRole("button", { name: "编辑" });
    await user.click(trigger);
    const dialog = screen.getByRole("dialog");
    expect(within(dialog).getByRole("button", { name: "保存" })).toBeDisabled();
    await user.click(within(dialog).getByRole("checkbox", { name: /高级组/ }));
    vi.mocked(api.updateAccessKey!).mockRejectedValueOnce(
      new Error("save rejected"),
    );
    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "save rejected",
    );
    expect(
      within(dialog).getByRole("checkbox", { name: /高级组/ }),
    ).toBeChecked();
    await user.click(within(dialog).getByRole("button", { name: "保存" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(api.updateAccessKey).toHaveBeenLastCalledWith(
      "management-fixture",
      "old",
      expect.objectContaining({
        metadata: {
          models: ["model-a"],
          scope: ["relay"],
          accountGroupIds: ["group-b"],
        },
      }),
    );
    expect(trigger).toHaveFocus();
    expect(screen.getByText("高级组")).toBeInTheDocument();
  });

  it("creates with local defaults, copies the key and does not persist plaintext in the browser", async () => {
    const { api } = fixture();
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    expect(await screen.findByText("暂无 API Key")).toBeInTheDocument();
    expect(screen.queryByText(/运营侧签发/)).not.toBeInTheDocument();
    const dialog = await openCreate(user);
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "我的客户端",
    );
    await user.click(
      within(dialog).getByRole("button", { name: "创建 API Key" }),
    );
    const result = await screen.findByRole("dialog", {
      name: "API Key 已生成",
    });
    expect(api.createAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      expect.objectContaining({
        displayName: "我的客户端",
        ownerId: "local",
        resolvedProjectId: "local",
        resolvedTenantId: "local",
        keyKind: "normal",
        expiresAt: null,
      }),
    );
    expect(
      within(result).getByRole("textbox", { name: "API Key" }),
    ).toHaveValue("sk-gw-fixture-secret");
    expect(
      within(result).getByRole("textbox", { name: "API Key" }),
    ).toHaveFocus();
    expect(
      within(result).getByRole("textbox", { name: "API Base URL" }),
    ).toHaveValue("https://gateway.test/v1");
    await user.click(
      within(result).getByRole("button", { name: "复制 API Key" }),
    );
    expect(await navigator.clipboard.readText()).toBe("sk-gw-fixture-secret");
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(
      screen.queryByDisplayValue("sk-gw-fixture-secret"),
    ).not.toBeInTheDocument();
    expect(JSON.stringify(window.localStorage)).not.toContain(
      "sk-gw-fixture-secret",
    );
    expect(screen.getByText("我的客户端")).toBeInTheDocument();
  });

  it("keeps server identity fields explicit and retains form input after failure", async () => {
    const { api } = fixture([], "server");
    vi.mocked(api.createAccessKey!).mockRejectedValue(
      new Error("creation rejected"),
    );
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    const dialog = await openCreate(user);
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "server-key",
    );
    expect(
      within(dialog).getByRole("button", { name: "创建 API Key" }),
    ).toBeDisabled();
    for (const label of ["归属 ID", "项目 ID", "租户 ID"])
      await user.type(
        within(dialog).getByRole("textbox", { name: label }),
        "real-id",
      );
    await user.selectOptions(
      within(dialog).getByRole("combobox", { name: "有效期" }),
      "30",
    );
    await user.click(
      within(dialog).getByRole("button", { name: "创建 API Key" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "creation rejected",
    );
    expect(within(dialog).getByRole("textbox", { name: "名称" })).toHaveValue(
      "server-key",
    );
    const input = vi.mocked(api.createAccessKey!).mock.calls[0][1];
    expect(Date.parse(input.expiresAt!)).toBeGreaterThan(Date.now());
    await user.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("button", { name: "新建 API Key" })).toHaveFocus();
  });

  it("filters real key states and requires confirmation before disabling", async () => {
    const { api } = fixture([
      key("active-key"),
      key("expired-key", { expiresAt: "2000-01-01T00:00:00Z" }),
      key("revoked-key", { revokedAt: "2026-01-01T00:00:00Z" }),
    ]);
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("active-key");
    await user.selectOptions(
      screen.getByRole("combobox", { name: "密钥状态" }),
      "expired",
    );
    expect(screen.queryByText("active-key")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "轮换" })).toBeDisabled();
    await user.selectOptions(
      screen.getByRole("combobox", { name: "密钥状态" }),
      "active",
    );
    await user.click(screen.getByRole("button", { name: "停用" }));
    expect(api.setAccessKeyEnabled).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    await user.click(screen.getByRole("button", { name: "停用" }));
    await user.click(screen.getByRole("button", { name: "确认停用" }));
    await waitFor(() =>
      expect(api.setAccessKeyEnabled).toHaveBeenCalledWith(
        "management-fixture",
        "active-key",
        false,
      ),
    );
    expect(await screen.findByText("没有匹配的密钥")).toBeInTheDocument();
  });

  it("paginates long lists and searches across all pages", async () => {
    const { api } = fixture(
      Array.from({ length: 25 }, (_, index) => key(`key-${index}`)),
    );
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("key-0");
    expect(screen.getAllByRole("row")).toHaveLength(21);
    await user.click(screen.getByRole("button", { name: "下一页" }));
    expect(screen.getAllByRole("row")).toHaveLength(6);
    await user.type(screen.getByRole("textbox", { name: "搜索密钥" }), "key-0");
    expect(screen.getByText("key-0")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "下一页" }),
    ).not.toBeInTheDocument();
  });

  it("blocks duplicate submissions and discards a late secret after leaving the workspace", async () => {
    const { api } = fixture();
    const pending = deferred<ConsoleAccessKey>();
    vi.mocked(api.createAccessKey!).mockReturnValue(pending.promise);
    const user = userEvent.setup();
    const view = renderWithProviders(<Harness api={api} />);
    const dialog = await openCreate(user);
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "pending",
    );
    fireEvent.submit(dialog.querySelector("form")!);
    fireEvent.submit(dialog.querySelector("form")!);
    expect(api.createAccessKey).toHaveBeenCalledTimes(1);
    view.unmount();
    await act(async () =>
      pending.resolve(key("late", { token: "must-not-render" })),
    );
    expect(
      screen.queryByDisplayValue("must-not-render"),
    ).not.toBeInTheDocument();
  });

  it("leaves read-only users unable to create keys", async () => {
    const { api } = fixture();
    renderWithProviders(<Harness api={api} locked />);
    await screen.findByText("暂无 API Key");
    expect(screen.getByRole("button", { name: "新建 API Key" })).toBeDisabled();
  });

  it("reveals a replacement only after confirmed rotation and preserves keyboard focus", async () => {
    const { api } = fixture([key("rotate-me")]);
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    await screen.findByText("rotate-me");
    await user.click(screen.getByRole("button", { name: "轮换" }));
    expect(api.rotateAccessKey).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "确认轮换" }));
    const result = await screen.findByRole("dialog", {
      name: "API Key 已生成",
    });
    expect(screen.getAllByRole("dialog")).toHaveLength(1);
    expect(api.rotateAccessKey).toHaveBeenCalledWith(
      "management-fixture",
      "rotate-me",
    );
    await waitFor(() =>
      expect(
        within(result).getByRole("textbox", { name: "API Key" }),
      ).toHaveFocus(),
    );
    await user.click(within(result).getByRole("button", { name: "完成" }));
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "新建 API Key" }),
      ).toHaveFocus(),
    );
  });

  it("does not invent a key when a create response omits plaintext", async () => {
    const { api } = fixture();
    vi.mocked(api.createAccessKey!).mockResolvedValue(key("missing-token"));
    const user = userEvent.setup();
    renderWithProviders(<Harness api={api} />);
    const dialog = await openCreate(user);
    await user.type(
      within(dialog).getByRole("textbox", { name: "名称" }),
      "missing",
    );
    await user.click(
      within(dialog).getByRole("button", { name: "创建 API Key" }),
    );
    expect(await within(dialog).findByRole("alert")).toHaveTextContent(
      "未返回明文",
    );
    expect(
      screen.queryByRole("dialog", { name: "API Key 已生成" }),
    ).not.toBeInTheDocument();
  });

  it("accepts legacy catalogs without granting inferred local defaults", () => {
    const parsed = consoleAccessCatalogSchema.parse({});
    expect(parsed.storageMode).toBeUndefined();
    expect(newAccessKeyDraft(parsed).resolvedTenantId).toBe("");
    expect(
      accessKeyStatus(key("expired", { expiresAt: "2000-01-01T00:00:00Z" })),
    ).toBe("expired");
  });
});
