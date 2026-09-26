import { act, render, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useEffect } from "react";
import { useGatewayDesktopState } from "./useGatewayDesktopState";
import * as http from "../lib/http";
import * as tauri from "../lib/tauri";
import type { GatewayDesktopState, GatewayProcessSnapshot } from "../lib/types";

vi.mock("../lib/http", () => ({
  fetchGatewayHealth: vi.fn(),
  fetchGatewayModels: vi.fn(),
  fetchGatewayReady: vi.fn(),
  gatewayBaseUrl: vi.fn((port?: number | null) => `http://127.0.0.1:${port ?? 4200}`),
  runGatewayChatCompletionTest: vi.fn(),
}));

vi.mock("../lib/tauri", () => ({
  checkGatewayProfilePaths: vi.fn(),
  deleteProfile: vi.fn(),
  getGatewayProcessSnapshot: vi.fn(),
  getGatewayUiRuntimeInfo: vi.fn(),
  isTauriRuntime: vi.fn(),
  listProfiles: vi.fn(),
  loadProfile: vi.fn(),
  openGatewayLogDirectory: vi.fn(),
  readGatewayLogTail: vi.fn(),
  saveProfile: vi.fn(),
  startGatewaySidecar: vi.fn(),
  stopGatewaySidecar: vi.fn(),
}));

const stoppedSnapshot: GatewayProcessSnapshot = {
  running: false,
  pid: null,
  port: null,
  profileName: null,
  logPath: null,
  startedAt: null,
  startupState: null,
  shutdownState: null,
  lastError: null,
  recentLogLines: [],
};

const readySnapshot: GatewayProcessSnapshot = {
  running: true,
  pid: 4242,
  port: 4200,
  profileName: "local-default",
  logPath: null,
  startedAt: "2026-07-26T00:00:00.000Z",
  startupState: "ready",
  shutdownState: null,
  lastError: null,
  recentLogLines: [],
};

function StateProbe({ onState }: { onState?: (state: GatewayDesktopState) => void }) {
  const state = useGatewayDesktopState();
  useEffect(() => {
    onState?.(state);
  }, [onState, state]);
  return <output data-testid="notice">{state.notice?.message ?? ""}</output>;
}

function configureDesktopMocks(healthOk: boolean) {
  vi.mocked(tauri.isTauriRuntime).mockReturnValue(true);
  vi.mocked(tauri.getGatewayUiRuntimeInfo).mockResolvedValue({
    appName: "Gateway UI",
    themeFamily: "NeuroTerminal",
    gatewayMode: "headless-first",
  });
  vi.mocked(tauri.listProfiles).mockResolvedValue([]);
  vi.mocked(tauri.readGatewayLogTail).mockResolvedValue({ path: null, lines: [] });
  vi.mocked(tauri.saveProfile).mockResolvedValue(undefined);
  vi.mocked(tauri.startGatewaySidecar).mockResolvedValue(readySnapshot);
  vi.mocked(tauri.getGatewayProcessSnapshot).mockResolvedValue(stoppedSnapshot);
  vi.mocked(http.fetchGatewayHealth).mockResolvedValue({
    ok: healthOk,
    status: healthOk ? 200 : 0,
    durationMs: 1,
    data: healthOk ? { status: "ok" } : undefined,
    error: healthOk ? undefined : "connection refused",
  });
  vi.mocked(http.fetchGatewayReady).mockResolvedValue({
    ok: healthOk,
    status: healthOk ? 200 : 0,
    durationMs: 1,
    data: healthOk ? { ready: true } : undefined,
    error: healthOk ? undefined : "connection refused",
  });
}

describe("useGatewayDesktopState auto-start", () => {
  beforeEach(() => {
    configureDesktopMocks(false);
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("starts the packaged gateway sidecar once when no backend health endpoint is reachable", async () => {
    render(<StateProbe />);

    await waitFor(() => expect(tauri.startGatewaySidecar).toHaveBeenCalledWith("local-default"));
    expect(tauri.startGatewaySidecar).toHaveBeenCalledTimes(1);
    expect(tauri.saveProfile).toHaveBeenCalledWith(
      expect.objectContaining({ name: "local-default", port: 4200 }),
    );
  });

  it("does not auto-start a sidecar when an existing backend is healthy", async () => {
    configureDesktopMocks(true);
    render(<StateProbe />);

    await waitFor(() => expect(http.fetchGatewayHealth).toHaveBeenCalled());
    expect(tauri.startGatewaySidecar).not.toHaveBeenCalled();
  });

  it("invalidates profile-bound probes and marks a persisted draft clean", async () => {
    configureDesktopMocks(true);
    const { result } = renderHook(() => useGatewayDesktopState());
    await waitFor(() => expect(result.current.healthProbe?.ok).toBe(true));
    const draft = { ...result.current.draftProfile, name: "work-profile" };

    act(() => result.current.updateDraftProfile(draft));
    expect(result.current.healthProbe).toBeUndefined();
    expect(result.current.readyProbe).toBeUndefined();
    expect(result.current.hasUnsavedProfileChanges).toBe(true);
    vi.mocked(tauri.listProfiles).mockResolvedValue(["work-profile"]);
    vi.mocked(tauri.loadProfile).mockResolvedValue(draft);

    await act(async () => result.current.saveDraftProfile());
    expect(tauri.saveProfile).toHaveBeenCalledWith(draft);
    expect(tauri.loadProfile).toHaveBeenCalledWith("work-profile");
    expect(result.current.selectedProfileName).toBe("work-profile");
    expect(result.current.hasUnsavedProfileChanges).toBe(false);
    expect(result.current.busy).toBe(false);
    expect(result.current.notice?.tone).toBe("success");
  });

  it("retains an unsaved draft and clears busy after persistence fails", async () => {
    configureDesktopMocks(true);
    const { result } = renderHook(() => useGatewayDesktopState());
    await waitFor(() => expect(result.current.profileNames).toEqual(["local-default"]));
    const draft = { ...result.current.draftProfile, name: "unsaved-profile" };
    act(() => result.current.updateDraftProfile(draft));
    vi.mocked(tauri.saveProfile).mockRejectedValueOnce(new Error("storage unavailable"));

    await act(async () => result.current.saveDraftProfile());
    expect(result.current.draftProfile).toEqual(draft);
    expect(result.current.selectedProfileName).toBe("local-default");
    expect(result.current.hasUnsavedProfileChanges).toBe(true);
    expect(result.current.busy).toBe(false);
    expect(result.current.notice?.tone).toBe("danger");
  });

  it("resets the default profile without deleting a persisted profile", async () => {
    configureDesktopMocks(true);
    const { result } = renderHook(() => useGatewayDesktopState());
    await waitFor(() => expect(result.current.profileNames).toEqual(["local-default"]));
    const initial = result.current.draftProfile;
    act(() => result.current.updateDraftProfile({ ...initial, port: 4300 }));

    await act(async () => result.current.deleteSelectedProfile());
    expect(tauri.deleteProfile).not.toHaveBeenCalled();
    expect(result.current.draftProfile).toEqual(initial);
    expect(result.current.hasUnsavedProfileChanges).toBe(false);
  });
});
