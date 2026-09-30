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
});
