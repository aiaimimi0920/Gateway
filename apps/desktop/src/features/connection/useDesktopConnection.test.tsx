import { StrictMode, type ReactNode } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useDesktopConnection } from "./useDesktopConnection";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

describe("desktop connection ownership", () => {
  beforeEach(() => { vi.resetAllMocks(); });

  it("starts a fresh local connection once under StrictMode", async () => {
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_gateway_connection") return { mode: "local", serverUrl: "" };
      return "http://127.0.0.1:4200/ui/";
    });
    const wrapper = ({ children }: { children: ReactNode }) => <StrictMode>{children}</StrictMode>;
    const { result } = renderHook(useDesktopConnection, { wrapper });
    await waitFor(() => expect(result.current.address).toBe("http://127.0.0.1:4200/ui/"));
    expect(vi.mocked(invoke).mock.calls.filter(([command]) => command === "connect_gateway")).toEqual([
      ["connect_gateway", { settings: { mode: "local", serverUrl: "" } }],
    ]);
  });

  it("loads embedded settings without reconnecting or starting a sidecar", async () => {
    vi.mocked(invoke).mockResolvedValue({ mode: "existing", serverUrl: "https://gateway.example" });
    const { result } = renderHook(() => useDesktopConnection(false, "https://gateway.example"));
    await waitFor(() => expect(result.current.busy).toBe(false));
    expect(result.current.address).toBe("https://gateway.example");
    expect(result.current.settings.mode).toBe("existing");
    expect(invoke).toHaveBeenCalledExactlyOnceWith("load_gateway_connection");
  });

  it("keeps an unavailable existing endpoint selected and retries without local fallback", async () => {
    const settings = { mode: "existing", serverUrl: "https://gateway.example" } as const;
    vi.mocked(invoke).mockImplementation(async (command) => {
      if (command === "load_gateway_connection") return settings;
      throw new Error("Server unavailable");
    });
    const { result } = renderHook(useDesktopConnection);
    await waitFor(() => expect(result.current.error).toContain("Server unavailable"));
    expect(result.current.settings).toEqual(settings);
    await act(() => result.current.connect(result.current.settings));
    const calls = vi.mocked(invoke).mock.calls.filter(([command]) => command === "connect_gateway");
    expect(calls).toHaveLength(2);
    for (const [, args] of calls) expect(args).toEqual({ settings });
    expect(result.current.busy).toBe(false);
  });

  it("does not start a local backend when persisted settings cannot be read", async () => {
    vi.mocked(invoke).mockRejectedValue(new Error("Invalid connection settings"));
    const { result } = renderHook(useDesktopConnection);
    await waitFor(() => expect(result.current.error).toContain("Invalid connection settings"));
    expect(vi.mocked(invoke).mock.calls.every(([command]) => command === "load_gateway_connection")).toBe(true);
    expect(result.current.busy).toBe(false);
  });

  it("tests without applying, and retains the active address after a failed switch", async () => {
    vi.mocked(invoke).mockImplementation(async command => command === "load_gateway_connection"
      ? { mode: "local", serverUrl: "" } : "http://127.0.0.1:4200/ui/");
    const { result } = renderHook(useDesktopConnection);
    await waitFor(() => expect(result.current.address).toContain("4200"));
    const settings = { mode: "existing", serverUrl: "https://another.example" } as const;
    await act(() => result.current.test(settings));
    expect(invoke).toHaveBeenLastCalledWith("test_gateway_connection", { settings });
    expect(result.current.address).toBe("http://127.0.0.1:4200/ui/");
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Target unavailable"));
    await act(() => result.current.connect(settings));
    expect(result.current.address).toBe("http://127.0.0.1:4200/ui/");
    expect(result.current.error).toContain("Target unavailable");
  });

  it("clears stale test feedback when editing the connection draft without applying it", async () => {
    vi.mocked(invoke).mockImplementation(async command => command === "load_gateway_connection"
      ? { mode: "local", serverUrl: "" } : "http://127.0.0.1:4200/ui/");
    const { result } = renderHook(useDesktopConnection);
    await waitFor(() => expect(result.current.address).toContain("4200"));
    await act(() => result.current.test(result.current.settings));
    expect(result.current.tested).toBe(true);
    const next = { mode: "existing", serverUrl: "https://another.example" } as const;
    act(() => result.current.setSettings(next));
    expect(result.current.tested).toBe(false);
    vi.mocked(invoke).mockRejectedValueOnce(new Error("Target unavailable"));
    await act(() => result.current.test(next));
    expect(result.current.error).toContain("Target unavailable");
    const calls = vi.mocked(invoke).mock.calls.length;
    act(() => result.current.setSettings({ ...next, serverUrl: "https://retry.example" }));
    expect(result.current.error).toBe("");
    expect(result.current.address).toBe("http://127.0.0.1:4200/ui/");
    expect(invoke).toHaveBeenCalledTimes(calls);
  });
});
