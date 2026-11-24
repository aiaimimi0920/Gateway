// Review-only synthetic task; its image is intercepted locally by the browser checker.
import type { LumaCreationTask } from "../../src/api/console/luma-creation";

export function reviewPreview(task: LumaCreationTask, action: string): void {
  if (action === "respond") {
    task.responses = [{ invocationId: "review-1", status: "delivered" }];
    task.review = null; task.state = "running";
    task.sessionUpdate = { status: "working", status_line: "已收到答复，继续处理", events: [], has_more: false };
  } else if (action === "read" && !task.responses?.length) {
    task.state = "needs_input"; task.assets = [];
    task.review = { fingerprint: "synthetic-review-1", invocationId: "review-1", kind: "review",
      content: { params: { question: "请审阅这张图片" }, text: [{ target: "headline", label: "标题", text: "蓝色圆形" }] },
      answerFormat: { verdict_schema: { type: "object", properties: { decision: { enum: ["accept", "revise"] } },
        required: ["decision"] }, revision_targets: ["headline"] },
      show: ["review-image"], assets: [{ id: "review-image", kind: "image", state: "ready", url: "https://cdn.example.com/review.png" }] };
    task.usage = { credits: 9, settled: false, estimated: false, unresolvedEvents: 1, scope: "session" };
  } else if (action === "read") {
    task.sessionUpdate = { status: "your_turn", events: [{ type: "message", text: "已完成本次审阅" }], has_more: false };
  }
}
