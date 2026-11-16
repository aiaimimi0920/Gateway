import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { useGatewayDesktopState } from "./state/useGatewayDesktopState";
import type { GatewayDesktopState } from "./lib/types";

vi.mock("./state/useGatewayDesktopState", () => ({ useGatewayDesktopState: vi.fn() }));
vi.mock("./features/launcher/LauncherPanel", () => ({ LauncherPanel: () => <div>Launcher fixture</div> }));
vi.mock("./features/config/ConfigPanel", () => ({ ConfigPanel: () => null }));
vi.mock("./features/status/StatusPanel", () => ({ StatusPanel: () => null }));
vi.mock("./features/models/ModelsPanel", () => ({ ModelsPanel: () => null }));
vi.mock("./features/api-test/ApiTestPanel", () => ({ ApiTestPanel: () => null }));
vi.mock("./features/logs/LogsPanel", () => ({ LogsPanel: () => null }));
vi.mock("./features/settings/SettingsPanel", () => ({ SettingsPanel: () => null }));

// Page bodies are isolated above; this fixture supplies only the shell's state boundary.
function state(patch: Partial<GatewayDesktopState> = {}): GatewayDesktopState {
  return {
    processSnapshot: { running: false }, draftProfile: { name: "synthetic" }, baseUrl: "http://127.0.0.1:1",
    isTauriAvailable: true, busy: false, canStartGateway: true, startWillSaveDraftProfile: false,
    startGateway: vi.fn(async () => undefined), stopGateway: vi.fn(async () => undefined), refreshAll: vi.fn(async () => undefined),
    clearNotice: vi.fn(), ...patch
  } as GatewayDesktopState;
}
describe("desktop runtime shell controls", () => {
  beforeEach(() => vi.clearAllMocks());

  it("preserves start/stop eligibility and the actual task callback", async () => {
    const user = userEvent.setup(), current = state();
    vi.mocked(useGatewayDesktopState).mockReturnValue(current);
    const view = render(<App />);
    await user.click(screen.getByRole("button", { name: "启动 Gateway" }));
    expect(current.startGateway).toHaveBeenCalledOnce();
    vi.mocked(useGatewayDesktopState).mockReturnValue({ ...current, processSnapshot: { ...current.processSnapshot, running: true } });
    view.rerender(<App />);
    await user.click(screen.getByRole("button", { name: "停止 Gateway" }));
    expect(current.stopGateway).toHaveBeenCalledOnce();
    expect(screen.getByText("RUNNING")).toHaveClass("n-badge");
  });

  it.each([{ busy: true }, { isTauriAvailable: false }, { canStartGateway: false }])("keeps disallowed starts disabled: %j", async patch => {
    const current = state(patch);
    vi.mocked(useGatewayDesktopState).mockReturnValue(current);
    render(<App />);
    const action = screen.getByRole("button", { name: "启动 Gateway" });
    expect(action).toHaveClass("n-button"); expect(action).toBeDisabled();
    await userEvent.click(action); expect(current.startGateway).not.toHaveBeenCalled();
  });

  it("keeps truthful runtime feedback and dismissal under application ownership", async () => {
    const current = state({ notice: { tone: "danger", message: "测试启动失败" }, startWillSaveDraftProfile: true });
    vi.mocked(useGatewayDesktopState).mockReturnValue(current);
    render(<App />);
    expect(screen.getByRole("alert")).toHaveTextContent("测试启动失败");
    expect(screen.getByText("启动会先保存当前草稿")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "关闭提示" }));
    expect(current.clearNotice).toHaveBeenCalledOnce();
    expect(screen.getByRole("region", { name: "Gateway desktop summary" }).querySelectorAll(".n-panel")).toHaveLength(3);
  });
});
