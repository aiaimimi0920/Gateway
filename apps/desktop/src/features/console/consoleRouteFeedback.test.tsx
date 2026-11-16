import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { buildConsoleRouteFeedback } from "./consoleRouteFeedback";

const diagnostic = { code: "identity_invalid", path: "/providers/0/id", message: "<script>repair required</script>" };
type Options = Parameters<typeof buildConsoleRouteFeedback>[0];
const options: Options = {
  activeRouteDiagnostics: [diagnostic],
  accountGroupSummaryError: "summary unavailable",
  draftDocumentState: { invalid: true },
  t: (zh) => zh,
};
function Feedback({ value = options }: { value?: Options }) {
  const feedback = buildConsoleRouteFeedback(value);
  return <>{feedback.activeDiagnosticsFeedback}{feedback.accountSummaryFeedback}{feedback.draftStructureNotice}</>;
}

describe("console route feedback", () => {
  it("uses standard notices and preserves diagnostics as text rather than markup", () => {
    const { container } = render(<Feedback />);
    const alert = screen.getByRole("alert", { name: "Active route diagnostics" });
    expect(alert).toHaveClass("n-notice", "n-tone-danger");
    expect(within(alert).getByRole("listitem")).toHaveTextContent(`${diagnostic.code} · ${diagnostic.path} · ${diagnostic.message}`);
    expect(alert.querySelector("script")).toBeNull();
    expect(container.querySelector(".nt-alert, .nt-validation-list")).toBeNull();
    expect(container.querySelectorAll(".n-notice")).toHaveLength(3);
  });

  it("keeps summary fallback and invalid-draft boundaries visible with warning semantics", () => {
    render(<Feedback />);
    const summary = screen.getByRole("status", { name: "Account summary status" });
    expect(summary).toHaveClass("n-tone-warning");
    expect(summary).toHaveTextContent("后端账号汇总暂不可用，当前显示本地可解析快照：summary unavailable");
    const draft = screen.getByText("当前 JSON 草稿不可解析").closest('[role="status"]');
    expect(draft).toHaveClass("n-notice", "n-tone-warning");
    expect(draft).toHaveTextContent("账号与分组视图当前回退到上一份可解析快照；请联系管理员修复路由草稿。");
  });

  it("keeps diagnostic identity and DOM through refresh, then clears only resolved feedback", () => {
    const { rerender } = render(<Feedback />);
    const alert = screen.getByRole("alert");
    const row = within(alert).getByRole("listitem");
    rerender(<Feedback value={{ ...options, activeRouteDiagnostics: [{ ...diagnostic }] }} />);
    expect(screen.getByRole("alert")).toBe(alert);
    expect(within(alert).getByRole("listitem")).toBe(row);
    rerender(<Feedback value={{ ...options, activeRouteDiagnostics: [] }} />);
    expect(screen.queryByRole("alert")).toBeNull();
    expect(screen.getByRole("status", { name: "Account summary status" })).toBeInTheDocument();
  });

  it("renders all diagnostic identities independently and retains complete English messages", () => {
    const diagnostics = [diagnostic, { ...diagnostic, path: "/providers/1/id" }];
    render(<Feedback value={{ ...options, activeRouteDiagnostics: diagnostics, t: (_zh, en) => en }} />);
    const alert = screen.getByRole("alert", { name: "Active route diagnostics" });
    expect(within(alert).getAllByRole("listitem")).toHaveLength(2);
    expect(alert).toHaveTextContent("2 active route diagnostic(s) require attention");
    expect(alert).toHaveTextContent("Contact an administrator to resolve these route diagnostics before saving a new revision.");
    expect(screen.getByRole("status", { name: "Account summary status" })).toHaveTextContent("Backend account summary unavailable; showing the locally parseable snapshot: summary unavailable");
  });

  it("does not add notices or explanatory copy when there is no failure", () => {
    const { container } = render(<Feedback value={{ ...options, activeRouteDiagnostics: [], accountGroupSummaryError: null, draftDocumentState: { invalid: false } }} />);
    expect(container).toBeEmptyDOMElement();
  });
});
