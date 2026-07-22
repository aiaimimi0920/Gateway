import { describe, expect, it } from "vitest";
import type { GatewayProfile } from "./types";
import { createGatewayDesktopCommands, type GatewayInvoke } from "./tauri";

const profile: GatewayProfile = {
  name: "selected",
  runtimeRole: "standalone",
  gatewayManagementToken: "management-secret",
  port: 45123,
  gatewayRedisUrl: "redis://127.0.0.1:46379",
  gatewayDatabaseUrl: null,
  gatewayRoutesFile: "routes.yaml",
  logLevel: "info",
  workingDirectory: null,
  extraEnv: [],
};

describe("Tauri command bridge", () => {
  it("maps desktop capability methods to the existing Tauri command contract", async () => {
    const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
    const invoke: GatewayInvoke = async <T,>(command: string, args?: Record<string, unknown>) => {
      calls.push({ command, args });
      const result: unknown = command === "load_profile" ? profile : undefined;
      return result as T;
    };
    const commands = createGatewayDesktopCommands(invoke);

    await commands.getGatewayUiRuntimeInfo();
    await commands.listProfiles();
    await expect(commands.loadProfile("selected")).resolves.toEqual(profile);
    await commands.saveProfile(profile);
    await commands.deleteProfile("selected");
    await commands.checkGatewayProfilePaths(profile);
    await commands.startGatewaySidecar("selected");
    await commands.stopGatewaySidecar();
    await commands.getGatewayProcessSnapshot();
    await commands.openGatewayLogDirectory();
    await commands.readGatewayLogTail(80);

    expect(calls).toEqual([
      { command: "get_gateway_ui_runtime_info", args: undefined },
      { command: "list_profiles", args: undefined },
      { command: "load_profile", args: { name: "selected" } },
      { command: "save_profile", args: { profile } },
      { command: "delete_profile", args: { name: "selected" } },
      { command: "check_gateway_profile_paths", args: { profile } },
      { command: "start_gateway_sidecar", args: { profileName: "selected" } },
      { command: "stop_gateway_sidecar", args: undefined },
      { command: "get_gateway_process_snapshot", args: undefined },
      { command: "open_gateway_log_directory", args: undefined },
      { command: "read_gateway_log_tail", args: { maxLines: 80 } },
    ]);
  });

  it("exposes every desktop command through stable compatibility wrappers", () => {
    const invoke: GatewayInvoke = async <T,>() => undefined as T;
    const commands = createGatewayDesktopCommands(invoke);

    expect(Object.keys(commands).sort()).toEqual(
      [
        "checkGatewayProfilePaths",
        "deleteProfile",
        "getGatewayProcessSnapshot",
        "getGatewayUiRuntimeInfo",
        "listProfiles",
        "loadProfile",
        "openGatewayLogDirectory",
        "readGatewayLogTail",
        "saveProfile",
        "startGatewaySidecar",
        "stopGatewaySidecar",
      ].sort(),
    );
  });
});
