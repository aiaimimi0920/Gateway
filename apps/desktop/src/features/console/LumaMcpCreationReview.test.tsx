import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import type { GatewayApiClient, GatewayApiRequestOptions } from "../../api/client";
import type { LumaCreationTask } from "../../api/console/luma-creation";
import type { LumaOAuthProps } from "./useLumaOAuth";
import { LumaMcpCreationPanel } from "./LumaMcpCreationPanel";
import { LumaMcpCreationReview } from "./LumaMcpCreationReview";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const consent = "已审阅内容，确认按此答复继续；可能使用剩余会话 credits";
function task(): LumaCreationTask {
  return { id: "luma-mcp-test", providerId: "mcp", credentialId: "saved", revision: "r1", kind: "image",
    budgetCredits: 20, state: "needs_input", sessionId: "s1", runId: "r1", ended: false, leaseUntil: 0, error: null,
    assets: [], usage: null, responses: [], review: { kind: "review", invocationId: "g1", fingerprint: "question-1",
      content: { params: { question: "Review <script>unsafe</script>" }, text: [{ target: "title", label: "标题", text: "新的标题" }] },
      answerFormat: { verdict_schema: { type: "object", required: ["decision"] }, revision_targets: ["title"] },
      show: ["a1"], assets: [{ id: "a1", kind: "image", state: "ready", url: "https://cdn.example.com/review.png" }] } };
}
const click = async (name: string) => { await act(async () => fireEvent.click(screen.getByRole("button", { name }))); };
const fill = () => {
  fireEvent.change(screen.getByLabelText("审阅答复（JSON）"), { target: { value: '{"decision":"accept"}' } });
  fireEvent.click(screen.getByLabelText(consent));
};
it("shows the original review and assets as text/media, then sends only the user's confirmed answer", async () => {
  const value = task(); const answer = vi.fn();
  const view = render(<LumaMcpCreationReview task={value} disabled={false} onAnswer={answer} />);
  expect(view.container.querySelector("script")).toBeNull();
  expect(screen.getByText(/Review <script>unsafe<\/script>/)).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "待审阅图片" })).toHaveAttribute("src", "https://cdn.example.com/review.png");
  expect(screen.getByRole("button", { name: "提交审阅答复" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("审阅答复（JSON）"), { target: { value: "{" } });
  expect(screen.getByText("请输入有效 JSON，最大 16 KiB。")).toBeInTheDocument();
  fill();
  fireEvent.change(screen.getByLabelText("审阅答复（JSON）"), { target: { value: '{"decision":"revise"}' } });
  expect(screen.getByLabelText(consent)).not.toBeChecked();
  fireEvent.click(screen.getByLabelText(consent)); await click("提交审阅答复");
  expect(answer).toHaveBeenCalledExactlyOnceWith({ invocationId: "g1", fingerprint: "question-1", result: { decision: "revise" } });
  expect(screen.queryByRole("button", { name: "提交审阅答复" })).not.toBeInTheDocument();
});
it.each(["unsupported", "format", "media", "page", "delivered", "unconfirmed", "ended", "session-ended"])("blocks unanswered or unsafe review state: %s", variant => {
  const value = task();
  if (variant === "unsupported") value.review!.kind = "unsupported";
  if (variant === "format") value.review!.answerFormat = null;
  if (variant === "media") value.review!.assets = [];
  if (variant === "page") value.sessionUpdate = { has_more: true };
  if (["delivered", "unconfirmed"].includes(variant)) value.responses = [{ invocationId: "g1", status: variant }];
  if (variant === "ended") value.ended = true;
  if (variant === "session-ended") value.sessionUpdate = { status: "ended", has_more: false };
  const answer = vi.fn(); render(<LumaMcpCreationReview task={value} disabled={false} onAnswer={answer} />);
  expect(screen.queryByRole("button", { name: "提交审阅答复" })).not.toBeInTheDocument();
  expect(answer).not.toHaveBeenCalled();
});
it("resets the draft and consent when a different question arrives", () => {
  const value = task(); const answer = vi.fn();
  const view = render(<LumaMcpCreationReview task={value} disabled={false} onAnswer={answer} />); fill();
  const next = task(); next.review!.fingerprint = "question-2"; next.review!.invocationId = "g2";
  view.rerender(<LumaMcpCreationReview task={next} disabled={false} onAnswer={answer} />);
  expect(screen.getByLabelText("审阅答复（JSON）")).toHaveValue("");
  expect(screen.getByLabelText(consent)).not.toBeChecked();
});
it.each(["approve", "confirm", "text_input", "select"])("allows an explicit JSON answer to %s without an invented answer format", async kind => {
  const value = task();
  value.review = { kind, invocationId: "q1", fingerprint: kind, answerFormat: null, show: [], assets: [],
    content: { message: "Continue?", ...(kind === "confirm" ? { destructive: true } : {}),
      ...(kind === "select" ? { min: 1, max: 1, options: [{ asset_id: "a1" }] } : {}) } };
  const answer = vi.fn();
  render(<LumaMcpCreationReview task={value} disabled={false} onAnswer={answer} />);
  if (kind === "confirm") expect(screen.getByText("此操作不可撤销。", { exact: true })).toBeInTheDocument();
  const result = kind === "select" ? ["a1"] : { decision: "accept" };
  fireEvent.change(screen.getByLabelText("审阅答复（JSON）"), { target: { value: JSON.stringify(result) } });
  fireEvent.click(screen.getByLabelText(consent)); await click("提交审阅答复");
  expect(answer).toHaveBeenCalledExactlyOnceWith({ invocationId: "q1", fingerprint: kind, result });
});
it("uses the normal management request and fences late answers after changing accounts", async () => {
  const value = task();
  const response = { revision: "r1", providerId: "mcp", credentialId: "saved", task: value };
  const request = vi.fn(async (_path: string, schema, _options?: GatewayApiRequestOptions) => schema.parse(response));
  const props: LumaOAuthProps = { client: { request } as GatewayApiClient, managementToken: "owner", secretGrant: "grant",
    providerId: "mcp", credentialId: "saved", expectedRevision: "r1", operations: new Map(), disabled: false,
    onRequestSecretAccess: vi.fn(), onSaved: vi.fn() };
  const view = render(<LumaMcpCreationPanel {...props} />);
  fireEvent.change(screen.getByLabelText("任务 ID"), { target: { value: value.id } }); await click("查询结果与费用");
  fill(); let resolve!: (value: unknown) => void;
  request.mockImplementationOnce(() => new Promise(done => { resolve = done; })); await click("提交审阅答复");
  expect(request.mock.calls[1][2]?.body).toEqual({ action: "respond", providerId: "mcp", credentialId: "saved", expectedRevision: "r1",
    taskId: value.id, chargeConfirmed: true, answer: { invocationId: "g1", fingerprint: "question-1", result: { decision: "accept" } } });
  view.rerender(<LumaMcpCreationPanel {...props} credentialId="other" />);
  expect(request.mock.calls[1][2]?.signal?.aborted).toBe(true);
  await act(async () => resolve(response));
  expect(screen.queryByLabelText("待审阅内容")).not.toBeInTheDocument();
  expect(screen.getByLabelText("任务 ID")).toHaveValue("");
});
it.each([[], ["other"], ["a1", "a1"], { ids: ["a1"] }])("rejects selections outside the offered set or limits: %j", result => {
  const value = task();
  value.review = { kind: "select", invocationId: "q1", fingerprint: "choice", answerFormat: null, show: [], assets: [],
    content: { min: 1, max: 2, options: [{ asset_id: "a1" }, { asset_id: "a2" }] } };
  const answer = vi.fn(); render(<LumaMcpCreationReview task={value} disabled={false} onAnswer={answer} />);
  fireEvent.change(screen.getByLabelText("审阅答复（JSON）"), { target: { value: JSON.stringify(result) } });
  fireEvent.click(screen.getByLabelText(consent));
  expect(screen.getByRole("button", { name: "提交审阅答复" })).toBeDisabled();
  expect(answer).not.toHaveBeenCalled();
});
