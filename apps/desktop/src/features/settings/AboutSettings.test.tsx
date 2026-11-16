import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { renderWithProviders } from "../console/BrowserConsoleApp.render-fixture";
import { AboutSettings } from "./AboutSettings";
import { NativeAboutSettings } from "./NativeAboutSettings";
import { checkPublicRelease, projectUrl } from "./aboutRelease";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
const initial = () => ({ preferences: { showButton: true, automatic: false, intervalHours: 24, channel: "stable", autoDownload: false, autoRestart: false, downloadSource: "" },
  currentVersion: "V1.0.0", canInstall: true, configDirectory: "C:/isolated/data", busy: false, release: null, downloaded: false, lastChecked: 0, error: null });
beforeEach(() => vi.mocked(invoke).mockReset());
afterEach(() => vi.unstubAllGlobals());

it("checks public releases without sending management credentials and renders notes as text", async () => {
  const fetch = vi.fn().mockResolvedValue(new Response(JSON.stringify([{ tag_name: "V1.2.3", html_url: `${projectUrl}/releases/tag/V1.2.3`, body: "<script>unsafe()</script>", draft: false, prerelease: false }])));
  vi.stubGlobal("fetch", fetch);
  renderWithProviders(<AboutSettings />);
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "检查更新" }));
  expect(await screen.findByText("最新正式版本：V1.2.3")).toBeVisible();
  expect(fetch.mock.calls[0][1]).toMatchObject({ credentials: "omit", headers: { Accept: "application/vnd.github+json" } });
  await user.click(screen.getByRole("button", { name: "查看" }));
  expect(screen.getByRole("dialog")).toHaveTextContent("<script>unsafe()</script>");
  expect(document.querySelector("script")).toBeNull();
  expect(invoke).not.toHaveBeenCalled();
});

it("reports GitHub throttling instead of claiming the installed version is current", async () => {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue(new Response("rate limit", { status: 403 })));
  await expect(checkPublicRelease(new AbortController().signal)).rejects.toThrow("GitHub 请求受限");
});

it("persists native automatic check settings with safe dependent defaults", async () => {
  const state = initial();
  vi.mocked(invoke).mockImplementation(async (_command, args) => {
    if (args && "action" in args && args.action === "save") state.preferences = args.preferences as typeof state.preferences;
    return structuredClone(state);
  });
  renderWithProviders(<NativeAboutSettings />);
  await screen.findByText("V1.0.0");
  const row = document.querySelector('[data-setting-id="auto-update"]') as HTMLElement;
  await userEvent.setup().click(within(row).getByRole("radio", { name: "开启" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("gateway_about_action", { action: "save", preferences: { ...initial().preferences, automatic: true } }));
  expect(state.preferences.autoDownload).toBe(false);
  expect(state.preferences.autoRestart).toBe(false);
});

it("opens only the native fixed configuration directory and confirms installation", async () => {
  vi.mocked(invoke).mockResolvedValue({ ...initial(), downloaded: true });
  renderWithProviders(<NativeAboutSettings />);
  await screen.findByText("V1.0.0");
  const user = userEvent.setup();
  await user.click(screen.getByRole("button", { name: "打开" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("gateway_about_action", { action: "openDirectory", preferences: null }));
  await user.click(screen.getByRole("button", { name: "重启并更新" }));
  expect(screen.getByRole("dialog")).toHaveTextContent("远程服务器不受影响");
  expect(vi.mocked(invoke).mock.calls.some(([, args]) => args && "action" in args && args.action === "install")).toBe(false);
  await user.click(screen.getByRole("button", { name: "确认更新" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("gateway_about_action", { action: "install", preferences: null }));
});

it("does not enable installation for development or hand-test packages", async () => {
  vi.mocked(invoke).mockResolvedValue({ ...initial(), canInstall: false, downloaded: true });
  renderWithProviders(<NativeAboutSettings />);
  expect(await screen.findByRole("button", { name: "重启并更新" })).toBeDisabled();
});
