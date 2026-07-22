import { invoke } from "@tauri-apps/api/core";
import type {
  GatewayLogTail,
  GatewayProfilePathCheck,
  GatewayProcessSnapshot,
  GatewayProfile,
  GatewayUiRuntimeInfo,
} from "./types";

type TauriWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

export type GatewayInvoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  const tauriWindow = window as TauriWindow;
  return Boolean(tauriWindow.__TAURI__ || tauriWindow.__TAURI_INTERNALS__);
}

const invokeGateway: GatewayInvoke = async <T,>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T> => {
  if (!isTauriRuntime()) {
    throw new Error("当前页面不在 Tauri 运行时中，无法调用本地 Gateway 桌面命令。");
  }
  return invoke<T>(command, args);
};

export function createGatewayDesktopCommands(invokeCommand: GatewayInvoke = invokeGateway) {
  return {
    getGatewayUiRuntimeInfo: () =>
      invokeCommand<GatewayUiRuntimeInfo>("get_gateway_ui_runtime_info"),
    listProfiles: () => invokeCommand<string[]>("list_profiles"),
    loadProfile: (name: string) => invokeCommand<GatewayProfile>("load_profile", { name }),
    saveProfile: (profile: GatewayProfile) =>
      invokeCommand<void>("save_profile", { profile }),
    deleteProfile: (name: string) => invokeCommand<void>("delete_profile", { name }),
    checkGatewayProfilePaths: (profile: GatewayProfile) =>
      invokeCommand<GatewayProfilePathCheck>("check_gateway_profile_paths", { profile }),
    startGatewaySidecar: (profileName: string) =>
      invokeCommand<GatewayProcessSnapshot>("start_gateway_sidecar", { profileName }),
    stopGatewaySidecar: () =>
      invokeCommand<GatewayProcessSnapshot>("stop_gateway_sidecar"),
    getGatewayProcessSnapshot: () =>
      invokeCommand<GatewayProcessSnapshot>("get_gateway_process_snapshot"),
    openGatewayLogDirectory: () =>
      invokeCommand<string>("open_gateway_log_directory"),
    readGatewayLogTail: (maxLines = 160) =>
      invokeCommand<GatewayLogTail>("read_gateway_log_tail", { maxLines }),
  };
}

export type GatewayDesktopCommands = ReturnType<typeof createGatewayDesktopCommands>;

export const gatewayDesktopCommands = createGatewayDesktopCommands();

export function getGatewayUiRuntimeInfo(): Promise<GatewayUiRuntimeInfo> {
  return gatewayDesktopCommands.getGatewayUiRuntimeInfo();
}

export function listProfiles(): Promise<string[]> {
  return gatewayDesktopCommands.listProfiles();
}

export function loadProfile(name: string): Promise<GatewayProfile> {
  return gatewayDesktopCommands.loadProfile(name);
}

export function saveProfile(profile: GatewayProfile): Promise<void> {
  return gatewayDesktopCommands.saveProfile(profile);
}

export function deleteProfile(name: string): Promise<void> {
  return gatewayDesktopCommands.deleteProfile(name);
}

export function checkGatewayProfilePaths(profile: GatewayProfile): Promise<GatewayProfilePathCheck> {
  return gatewayDesktopCommands.checkGatewayProfilePaths(profile);
}

export function startGatewaySidecar(profileName: string): Promise<GatewayProcessSnapshot> {
  return gatewayDesktopCommands.startGatewaySidecar(profileName);
}

export function stopGatewaySidecar(): Promise<GatewayProcessSnapshot> {
  return gatewayDesktopCommands.stopGatewaySidecar();
}

export function getGatewayProcessSnapshot(): Promise<GatewayProcessSnapshot> {
  return gatewayDesktopCommands.getGatewayProcessSnapshot();
}

export function openGatewayLogDirectory(): Promise<string> {
  return gatewayDesktopCommands.openGatewayLogDirectory();
}

export function readGatewayLogTail(maxLines = 160): Promise<GatewayLogTail> {
  return gatewayDesktopCommands.readGatewayLogTail(maxLines);
}
