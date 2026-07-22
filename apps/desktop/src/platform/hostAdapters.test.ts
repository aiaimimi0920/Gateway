import { render, screen } from "@testing-library/react";
import { createElement } from "react";
import { useInRouterContext } from "react-router-dom";
import { describe, expect, it } from "vitest";
import { createBrowserHost } from "./browserHost";
import { createHostAdapter } from "./createHostAdapter";
import { HostProvider } from "./HostProvider";
import { createTauriHost } from "./tauriHost";

function RouterPresence() {
  return createElement("output", null, String(useInRouterContext()));
}

describe("Gateway host adapters", () => {
  it("uses window.location.origin for the browser API origin", () => {
    const host = createBrowserHost();

    expect(host.kind).toBe("browser");
    expect(host.apiOrigin).toBe(window.location.origin);
    expect(host.routing).toEqual({ mode: "browser", basename: "/ui" });
    expect(host.capabilities.sidecarLifecycle).toBe(false);
  });

  it("uses the selected loopback sidecar URL for Tauri", () => {
    const host = createTauriHost("http://127.0.0.1:44200/");

    expect(host.kind).toBe("tauri");
    expect(host.apiOrigin).toBe("http://127.0.0.1:44200");
    expect(host.routing).toEqual({ mode: "hash" });
    expect(host.capabilities.sidecarLifecycle).toBe(true);
    expect(host.desktop).toBeDefined();
  });

  it("derives the Tauri sidecar origin from the selected profile port", () => {
    const host = createTauriHost({ selectedProfile: { port: 45123 } });

    expect(host.apiOrigin).toBe("http://127.0.0.1:45123");
    expect(host.desktop?.listProfiles).toBeTypeOf("function");
    expect(
      createHostAdapter({ target: "tauri", selectedProfile: { port: 45124 } }).apiOrigin,
    ).toBe("http://127.0.0.1:45124");
  });

  it("requires an explicit Tauri sidecar origin or selected profile", () => {
    expect(() => createHostAdapter({ target: "tauri" })).toThrow(/profile|origin/i);
  });

  it("selects the deterministic build target", () => {
    expect(createHostAdapter({ target: "web" }).kind).toBe("browser");
    expect(
      createHostAdapter({
        target: "tauri",
        tauriApiOrigin: "http://localhost:45123",
      }).apiOrigin,
    ).toBe("http://localhost:45123");
  });

  it("installs the target-specific router at the host boundary", () => {
    render(
      createElement(
        HostProvider,
        { adapter: createBrowserHost(), children: createElement(RouterPresence) },
      ),
    );

    expect(screen.getByText("true")).toBeInTheDocument();
  });

  it("rejects a non-loopback Tauri API origin", () => {
    expect(() => createTauriHost("https://gateway.example.com")).toThrow(/loopback/i);
  });
});
