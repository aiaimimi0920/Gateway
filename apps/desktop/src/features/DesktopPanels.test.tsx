import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { createDesktopPanelState } from "./desktopPanels.test-fixtures";
import { ApiTestPanel } from "./api-test/ApiTestPanel";
import { LogsPanel } from "./logs/LogsPanel";
import { ModelsPanel } from "./models/ModelsPanel";
import { StatusPanel } from "./status/StatusPanel";
import { LauncherPanel } from "./launcher/LauncherPanel";

describe("standard legacy desktop panels", () => {
  it("keeps API request field updates and eligibility at the state boundary", async () => {
    const user = userEvent.setup(), current = createDesktopPanelState();
    const view = render(<ApiTestPanel state={current} />);
    await user.type(screen.getByLabelText("Gateway API Key"), "s");
    expect(current.updateApiTestInput).toHaveBeenLastCalledWith({ ...current.apiTestInput, apiKey: "s" });
    await user.click(screen.getByRole("button", { name: "发送测试请求" }));
    expect(current.runApiTest).toHaveBeenCalledOnce();
    view.rerender(<ApiTestPanel state={{ ...current, apiTestInput: { ...current.apiTestInput, model: " " } }} />);
    expect(screen.getByRole("button", { name: "发送测试请求" })).toBeDisabled();
    view.rerender(<ApiTestPanel state={{ ...current, busy: true }} />);
    expect(screen.getByRole("button", { name: "发送测试请求" })).toBeDisabled();
  });

  it("renders real response context and failure payloads rather than success placeholders", () => {
    render(<ApiTestPanel state={createDesktopPanelState({
      apiTestResult: {
        ok: false, status: 401, durationMs: 4,
        endpoint: "/v1/chat/completions", error: "synthetic denied", profileName: "synthetic-profile", baseUrl: "http://127.0.0.1:1",
        model: "synthetic-model", requestedAt: "synthetic-time"
      }
    })} />);
    expect(screen.getByText("401 / 4ms")).toHaveClass("n-badge");
    expect(screen.getByText(/synthetic denied/)).toBeInTheDocument();
    expect(screen.getByLabelText("API test request context")).toHaveTextContent("synthetic-profile");
  });

  it("preserves logging commands, path fallback and manual diagnostic copy", async () => {
    const user = userEvent.setup(), current = createDesktopPanelState({ diagnosticsReportText: "脱敏 synthetic report" });
    current.processSnapshot.logPath = "C:/synthetic/log.txt";
    const view = render(<LogsPanel state={current} />);
    for (const [label, action] of [["复制诊断", current.copyDiagnostics], ["复制日志路径", current.copyCurrentLogPath],
    ["打开日志目录", current.openLogDirectory], ["刷新日志", current.refreshLogs]] as const) {
      await user.click(screen.getByRole("button", { name: label })); expect(action).toHaveBeenCalledOnce();
    }
    expect(screen.getByText("脱敏 synthetic report")).toBeInTheDocument();
    view.rerender(<LogsPanel state={createDesktopPanelState({ isTauriAvailable: false })} />);
    expect(screen.getByRole("button", { name: "复制日志路径" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "打开日志目录" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "刷新日志" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "复制诊断" })).not.toBeDisabled();
  });

  it("uses the standard model table while retaining row keys and owner fallbacks", async () => {
    const user = userEvent.setup(), current = createDesktopPanelState({
      modelsProbe: {
        ok: true, status: 200, durationMs: 2,
        data: { data: [{ id: "one", owned_by: "OwnerA" }, { id: "two", ownedBy: "OwnerB" }] }
      }
    });
    render(<ModelsPanel state={current} />);
    const table = screen.getByRole("table", { name: "模型目录结果" });
    expect(table).toHaveClass("n-table");
    expect(within(table).getByText("OwnerA")).toBeInTheDocument(); expect(within(table).getByText("OwnerB")).toBeInTheDocument();
    expect(table.querySelector('[data-row-key="two"]')).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "读取模型" })); expect(current.refreshModels).toHaveBeenCalledOnce();
  });

  it("keeps degraded readiness distinct from healthy liveness and exposes actual errors", async () => {
    const user = userEvent.setup(), current = createDesktopPanelState({
      healthProbe: { ok: true, status: 200, durationMs: 2, data: { status: "ok" } },
      readyProbe: { ok: true, status: 200, durationMs: 4, data: { ready: true, degraded: true } },
    });
    current.processSnapshot.lastError = "synthetic startup failure";
    render(<StatusPanel state={current} />);
    expect(screen.getByText("HTTP 200")).toHaveClass("n-tone-success");
    expect(screen.getByText("degraded mode / HTTP 200")).toHaveClass("n-tone-warning");
    expect(screen.getByRole("alert")).toHaveTextContent("synthetic startup failure");
    await user.click(screen.getByRole("button", { name: "刷新检查" })); expect(current.refreshRuntimeSignals).toHaveBeenCalledOnce();
  });

  it("preserves launcher process eligibility and removes architecture marketing copy", async () => {
    const user = userEvent.setup(), current = createDesktopPanelState();
    const view = render(<LauncherPanel state={current} />);
    await user.click(screen.getByRole("button", { name: "启动 Gateway" })); expect(current.startGateway).toHaveBeenCalledOnce();
    expect(screen.getByRole("button", { name: "停止 sidecar" })).toBeDisabled();
    expect(screen.queryByText("Headless-first")).not.toBeInTheDocument();
    view.rerender(<LauncherPanel state={{ ...current, processSnapshot: { ...current.processSnapshot, running: true } }} />);
    expect(screen.getByRole("button", { name: "启动 Gateway" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "停止 sidecar" })); expect(current.stopGateway).toHaveBeenCalledOnce();
  });
});
