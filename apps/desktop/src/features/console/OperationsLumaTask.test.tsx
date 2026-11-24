import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { OperationsRequestTable } from "./OperationsRequestTable";
import { operationsRequestFixture } from "./OperationsWorkspace.fixtures";
vi.mock("../../i18n/UiLocaleProvider", () => ({ useUiLocale: () => ({ t: (zh: string) => zh }) }));
const id = "d290f1ee-6c54-4b01-90e6-d701748f0851";
const task = { id, model: "uni-1", type: "image", state: "completed", usage: { credits_charged: null }, mediaVerified: false };
function mount(value: unknown) {
  const row = { ...operationsRequestFixture(), status: "cancelled", routeTrace: { lumaAgentsGeneration: value } };
  render(<OperationsRequestTable rows={[row as ReturnType<typeof operationsRequestFixture>]} t={zh => zh} />);
  fireEvent.click(screen.getByRole("button", { name: `请求详情: ${row.id}` }));
}
it("shows supplier facts on cancelled requests without claiming local success or zero charge", () => {
  mount({ ...task, output: "private-url", prompt: "private-prompt" });
  expect(screen.getByLabelText("Luma 任务 ID")).toHaveValue(id);
  expect(screen.getByText("已完成")).toBeInTheDocument(); expect(screen.getByText("未验证")).toBeInTheDocument();
  expect(screen.getByText("未知")).toBeInTheDocument(); expect(screen.getByRole("button", { name: "复制 Luma 任务 ID" })).toBeInTheDocument();
  expect(screen.queryByText(/private-/)).not.toBeInTheDocument();
});
it.each([null, {}, { ...task, id: "../unsafe" }, { ...task, usage: { credits_charged: -1 } }])("ignores historical or invalid task observations", value => {
  mount(value); expect(screen.queryByLabelText("Luma 任务 ID")).not.toBeInTheDocument();
});
it("supports older task receipts without a media verification fact", () => {
  mount({ ...task, mediaVerified: undefined, usage: { credits_charged: 0 } });
  expect(screen.getByText("0")).toBeInTheDocument(); expect(screen.queryByText("本地媒体验证")).not.toBeInTheDocument();
});
