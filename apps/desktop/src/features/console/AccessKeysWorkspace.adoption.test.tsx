import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, it, vi } from "vitest";
import type { ConsoleApi } from "../../api/console";
import { deferred, renderWithProviders } from "./BrowserConsoleApp.render-fixture";
import { AccessWorkspaceHarness, advancedAffinity, advancedCatalog, advancedCredential } from "./AccessKeysWorkspace.adoption-fixtures";
import type { UseAccessDataResult } from "./useAccessData";
import { accessStatusTone } from "./accessWorkspaceFormatting";
import { SecretBanner } from "./AccessWorkspacePrimitives";

const t = (zh: string) => zh;
const block = (name: string) => within(screen.getByRole("heading", { name }).closest<HTMLElement>("article, .n-panel")!);
function receiver() {
  const catalog = structuredClone(advancedCatalog);
  const api = {
    getAccessCatalog: vi.fn(async () => structuredClone(catalog)),
    createAccessBundle: vi.fn(async () => catalog.bundles[0]),
    inspectAccessAffinity: vi.fn(async () => ({ affinity: advancedAffinity })),
    resetAccessAffinity: vi.fn(async () => ({ success: true, message: "cleared" })),
    rotateApiAccess: vi.fn(async () => ({ projectId: "project-a", tenantId: "tenant-a", project: {}, tenant: {}, apiKey: {}, token: "synthetic-project-secret" })),
    issueUserCredential: vi.fn(async () => ({ success: true, credential: advancedCredential, message: "issued" })),
    verifyUserCredential: vi.fn(async () => ({ valid: true, credential: { ...advancedCredential, status: "active" }, reason: "verified" })),
    revokeUserCredential: vi.fn(async () => ({ success: true, message: "revoked" })),
  };
  return { catalog, api, typed: api as unknown as ConsoleApi };
}
async function expand(user: ReturnType<typeof userEvent.setup>, name: string) {
  await user.click(screen.getByText("高级管理", { exact: true }));
  await user.click(screen.getByText(name, { exact: true }));
}
beforeEach(() => { window.localStorage.clear(); vi.restoreAllMocks(); });

it("keeps advanced management out of local mode and lazily mounts independent sections", async () => {
  const { typed, catalog } = receiver();
  let access!: UseAccessDataResult;
  const user = userEvent.setup();
  renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} onAccess={value => { access = value; }} />);
  await screen.findByText("暂无 API Key");
  expect(screen.queryByLabelText("Slug")).not.toBeInTheDocument();
  await expand(user, "权益包");
  await user.type(screen.getByLabelText("Slug"), "persisted-draft");
  await user.click(screen.getByText("会话粘性与项目密钥", { exact: true }));
  expect(screen.getByLabelText("Slug")).toHaveValue("persisted-draft");
  await user.click(screen.getByText("权益包", { exact: true }));
  expect(screen.queryByLabelText("Slug")).not.toBeInTheDocument();
  await user.click(screen.getByText("权益包", { exact: true }));
  expect(screen.getByLabelText("Slug")).toHaveValue("persisted-draft");
  catalog.storageMode = "local";
  await act(async () => access.refresh());
  await waitFor(() => expect(screen.queryByText("高级管理")).not.toBeInTheDocument());
});

it("preserves bundle serialization, failed drafts, explicit reset and stable fields on refresh", async () => {
  const { typed, api } = receiver();
  api.createAccessBundle.mockRejectedValueOnce(new Error("bundle rejected"));
  let access!: UseAccessDataResult;
  const user = userEvent.setup();
  renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} onAccess={value => { access = value; }} />);
  await screen.findByText("暂无 API Key"); await expand(user, "权益包");
  const form = block("新建权益包"), slug = form.getByLabelText("Slug");
  expect(form.getByRole("button", { name: "创建" })).toBeDisabled();
  fireEvent.change(slug, { target: { value: " bundle-new " } });
  fireEvent.change(form.getByLabelText("显示名"), { target: { value: " New Bundle " } });
  fireEvent.change(form.getByLabelText("计费模式"), { target: { value: " custom-mode " } });
  await user.click(form.getByRole("button", { name: "创建" }));
  await screen.findByText("bundle rejected");
  expect(api.createAccessBundle).toHaveBeenCalledWith("synthetic-admin", { projectId: null, slug: "bundle-new", displayName: "New Bundle", billingMode: "custom-mode", status: "active", description: null });
  expect(slug).toHaveValue(" bundle-new ");
  slug.focus(); await act(async () => access.refresh());
  await waitFor(() => expect(access.refreshing).toBe(false));
  expect(form.getByLabelText("Slug")).toBe(slug); expect(slug).toHaveFocus();
  await user.click(form.getByRole("button", { name: "重置" }));
  expect(slug).toHaveValue(""); expect(form.getByLabelText("计费模式")).toHaveValue("metered");
});

it("preserves affinity parameters, busy boundaries, failed reset data and explicit clear", async () => {
  const { typed, api } = receiver();
  const pending = deferred<{ affinity: typeof advancedAffinity }>();
  api.inspectAccessAffinity.mockReturnValueOnce(pending.promise);
  api.resetAccessAffinity.mockRejectedValueOnce(new Error("reset rejected"));
  const user = userEvent.setup(); renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} />);
  await screen.findByText("暂无 API Key"); await expand(user, "会话粘性与项目密钥");
  const form = block("会话粘性");
  fireEvent.change(form.getByLabelText("访问密钥 ID"), { target: { value: " key/a " } });
  fireEvent.change(form.getByLabelText("模型"), { target: { value: " model a " } });
  fireEvent.change(form.getByLabelText("会话键"), { target: { value: " session-a " } });
  await user.click(form.getByRole("button", { name: "查询粘性" }));
  expect(form.getByRole("button", { name: "清除粘性" })).toBeDisabled();
  expect(api.inspectAccessAffinity).toHaveBeenCalledWith("synthetic-admin", { accessKeyId: "key/a", model: "model a", explicitSessionKey: "session-a" });
  await act(async () => pending.resolve({ affinity: advancedAffinity }));
  await user.click(form.getByRole("button", { name: "清除粘性" }));
  await screen.findByText("reset rejected"); expect(form.getByText("provider-a")).toBeInTheDocument();
  await user.click(form.getByRole("button", { name: "清除粘性" }));
  await screen.findByText("cleared"); expect(form.queryByText("provider-a")).not.toBeInTheDocument();
  expect(form.getByText("尚无粘性记录")).toBeInTheDocument();
});

it("rotates only explicit project input and keeps one-shot plaintext dismissible", async () => {
  const { typed, api } = receiver();
  const user = userEvent.setup(); renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} />);
  await screen.findByText("暂无 API Key"); await expand(user, "会话粘性与项目密钥");
  const form = block("轮换项目 API 密钥");
  fireEvent.change(form.getByLabelText("项目 ID"), { target: { value: " project-a " } });
  fireEvent.change(form.getByLabelText("密钥名称"), { target: { value: " key-name " } });
  await user.click(form.getByRole("button", { name: "轮换" }));
  await screen.findByText("synthetic-project-secret");
  expect(api.rotateApiAccess).toHaveBeenCalledWith("synthetic-admin", { projectId: "project-a", name: "key-name", actorUserId: undefined });
  expect(screen.getByText(/该明文只会返回一次/)).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "我已保存" }));
  expect(screen.queryByText("synthetic-project-secret")).not.toBeInTheDocument();
  expect(form.getByText("tenant-a")).toBeInTheDocument();
});

it("preserves issue fallback, first verify scope and failed revoke diagnostics", async () => {
  const { typed, api } = receiver();
  api.revokeUserCredential.mockRejectedValueOnce(new Error("revoke rejected"));
  const user = userEvent.setup(); renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} />);
  await screen.findByText("暂无 API Key"); await expand(user, "用户凭据");
  const issue = block("签发凭据");
  fireEvent.change(issue.getByLabelText("用户 ID"), { target: { value: " user-a " } });
  fireEvent.change(issue.getByLabelText("有效天数"), { target: { value: "invalid" } });
  fireEvent.change(issue.getByLabelText("授权范围"), { target: { value: "chat.completions, responses" } });
  await user.click(issue.getByRole("button", { name: "签发" }));
  await screen.findByText("synthetic-credential-secret");
  expect(api.issueUserCredential).toHaveBeenCalledWith("synthetic-admin", { userId: "user-a", projectId: null, credentialType: "session", durationDays: 30, scope: ["chat.completions", "responses"] });
  await user.click(screen.getByRole("button", { name: "我已保存" }));
  const verify = block("校验与吊销");
  fireEvent.change(verify.getByLabelText("凭据明文"), { target: { value: " secret-a " } });
  fireEvent.change(verify.getByLabelText("校验范围"), { target: { value: "responses, ignored" } });
  await user.click(verify.getByRole("button", { name: "校验" }));
  await verify.findByText("有效"); expect(api.verifyUserCredential).toHaveBeenCalledWith("synthetic-admin", "secret-a", "responses");
  await user.click(verify.getByRole("button", { name: "吊销" }));
  await screen.findByText("revoke rejected"); expect(verify.getByText("有效")).toBeInTheDocument();
  expect(api.revokeUserCredential).toHaveBeenCalledWith("synthetic-admin", "secret-a", "revoked from console");
  await user.click(verify.getByRole("button", { name: "吊销" }));
  await verify.findByText("revoked"); expect(verify.queryByText("有效")).not.toBeInTheDocument();
});

it("uses standard controls and stable bundle rows with unique field associations", async () => {
  const { typed, catalog } = receiver();
  let access!: UseAccessDataResult;
  const user = userEvent.setup();
  renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} onAccess={value => { access = value; }} />);
  await screen.findByText("暂无 API Key"); await expand(user, "权益包");
  await user.click(screen.getByText("会话粘性与项目密钥", { exact: true }));
  await user.click(screen.getByText("用户凭据", { exact: true }));
  const table = screen.getByRole("table", { name: "权益包台账" });
  expect(table).toHaveClass("n-table");
  const row = table.querySelector('[data-row-key="bundle-a"]')!;
  expect(within(row as HTMLElement).getByText("active")).toHaveClass("n-badge", "n-tone-success");
  expect(within(row as HTMLElement).getByRole("rowheader")).toHaveTextContent("Bundle A");
  expect(within(row as HTMLElement).getByText("2")).toBeInTheDocument();
  const fields = [...document.querySelectorAll<HTMLInputElement>(".nt-access-fields input")];
  expect(fields).toHaveLength(19);
  expect(new Set(fields.map(field => field.id)).size).toBe(19);
  for (const field of fields) { expect(field).toHaveClass("n-input"); expect(field.labels).toHaveLength(1); }
  catalog.bundles[0].displayName = "Updated Bundle";
  await act(async () => access.refresh());
  await screen.findByText("Updated Bundle");
  expect(table.querySelector('[data-row-key="bundle-a"]')).toBe(row);
  expect(document.querySelector(".nt-access-workspace .nt-input, .nt-access-workspace .nt-btn, .nt-access-workspace .nt-table")).toBeNull();
});

it("retains action permissions without introducing draft submission on Enter", async () => {
  const { typed, api } = receiver();
  const user = userEvent.setup(); renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} locked />);
  await screen.findByText("暂无 API Key"); await expand(user, "权益包");
  const form = block("新建权益包");
  fireEvent.change(form.getByLabelText("Slug"), { target: { value: "valid-slug" } });
  fireEvent.change(form.getByLabelText("显示名"), { target: { value: "valid-name" } });
  expect(form.getByRole("button", { name: "创建" })).toBeDisabled();
  expect(form.getByRole("button", { name: "重置" })).toBeDisabled();
  form.getByLabelText("Slug").focus(); await user.keyboard("{Enter}");
  expect(api.createAccessBundle).not.toHaveBeenCalled();
  expect(form.getByLabelText("Slug")).not.toBeDisabled();
});

it("keeps one-shot copy pending and failure truthful without hiding plaintext", async () => {
  const user = userEvent.setup(), pending = deferred<void>();
  const write = vi.spyOn(navigator.clipboard, "writeText").mockReturnValueOnce(pending.promise);
  const dismiss = vi.fn();
  renderWithProviders(<SecretBanner t={t} secret={{ kind: "api-access", label: "Synthetic key", secret: "synthetic-secret" }} onDismiss={dismiss} />);
  const copy = screen.getByRole("button", { name: "复制明文" });
  await user.click(copy);
  expect(copy).toBeDisabled(); expect(copy).toHaveAttribute("aria-busy", "true");
  expect(write).toHaveBeenCalledWith("synthetic-secret");
  expect(screen.queryByText("已复制")).not.toBeInTheDocument();
  await act(async () => pending.reject(new Error("clipboard denied")));
  expect(screen.getByText("复制失败")).toBeInTheDocument();
  expect(screen.getByText("synthetic-secret")).toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "我已保存" }));
  expect(dismiss).toHaveBeenCalledOnce();
});

it.each([
  ["权益包", "新建权益包", "createAccessBundle", "创建", { Slug: "bundle", "显示名": "Bundle" }],
  ["会话粘性与项目密钥", "会话粘性", "inspectAccessAffinity", "查询粘性", { "访问密钥 ID": "key", "模型": "model" }],
  ["会话粘性与项目密钥", "轮换项目 API 密钥", "rotateApiAccess", "轮换", { "项目 ID": "project" }],
  ["用户凭据", "签发凭据", "issueUserCredential", "签发", { "用户 ID": "user" }],
  ["用户凭据", "校验与吊销", "verifyUserCredential", "校验", { "凭据明文": "synthetic-key" }],
] as const)("keeps the %s / %s action node and label stable while pending", async (section, heading, operation, label, values) => {
  const { typed, api } = receiver(), pending = deferred<never>();
  api[operation].mockReturnValueOnce(pending.promise);
  const user = userEvent.setup();
  renderWithProviders(<AccessWorkspaceHarness api={typed} t={t} />);
  await screen.findByText("暂无 API Key"); await expand(user, section);
  const form = block(heading);
  for (const [name, value] of Object.entries(values)) fireEvent.change(form.getByLabelText(name), { target: { value } });
  const button = form.getByRole("button", { name: label });
  await user.click(button);
  expect(api[operation]).toHaveBeenCalledOnce();
  expect(form.getByRole("button", { name: label })).toBe(button);
  expect(button).toBeDisabled(); expect(button).toHaveAttribute("aria-busy", "true");
  expect(button).toHaveClass("n-loading");
  await act(async () => pending.reject(new Error("synthetic pending failure")));
  await screen.findByText("synthetic pending failure");
  expect(button).not.toBeDisabled(); expect(button).not.toHaveAttribute("aria-busy");
});

it.each([
  ["ACTIVE", "success"], ["enabled", "success"], ["revoked", "danger"], ["disabled", "danger"],
  ["expired", "danger"], ["pending", "warning"], ["suspended", "warning"], ["custom-state", "neutral"], [null, "neutral"],
])("preserves the status role of %s", (status, expected) => {
  expect(accessStatusTone(status)).toBe(expected);
});
