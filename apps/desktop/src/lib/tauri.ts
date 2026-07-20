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

export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }
  const tauriWindow = window as TauriWindow;
  return Boolean(tauriWindow.__TAURI__ || tauriWindow.__TAURI_INTERNALS__);
}

async function invokeGateway<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauriRuntime()) {
    throw new Error("当前页面不在 Tauri 运行时中，无法调用本地 Gateway 桌面命令。");
  }
  return invoke<T>(command, args);
}

export function getGatewayUiRuntimeInfo(): Promise<GatewayUiRuntimeInfo> {
  return invokeGateway<GatewayUiRuntimeInfo>("get_gateway_ui_runtime_info");
}

export function listProfiles(): Promise<string[]> {
  return invokeGateway<string[]>("list_profiles");
}

export function loadProfile(name: string): Promise<GatewayProfile> {
  return invokeGateway<GatewayProfile>("load_profile", { name });
}

export function saveProfile(profile: GatewayProfile): Promise<void> {
  return invokeGateway<void>("save_profile", { profile });
}

export function deleteProfile(name: string): Promise<void> {
  return invokeGateway<void>("delete_profile", { name });
}

export function checkGatewayProfilePaths(profile: GatewayProfile): Promise<GatewayProfilePathCheck> {
  return invokeGateway<GatewayProfilePathCheck>("check_gateway_profile_paths", { profile });
}

export function startGatewaySidecar(profileName: string): Promise<GatewayProcessSnapshot> {
  return invokeGateway<GatewayProcessSnapshot>("start_gateway_sidecar", { profileName });
}

export function stopGatewaySidecar(): Promise<GatewayProcessSnapshot> {
  return invokeGateway<GatewayProcessSnapshot>("stop_gateway_sidecar");
}

export function getGatewayProcessSnapshot(): Promise<GatewayProcessSnapshot> {
  return invokeGateway<GatewayProcessSnapshot>("get_gateway_process_snapshot");
}

export function openGatewayLogDirectory(): Promise<string> {
  return invokeGateway<string>("open_gateway_log_directory");
}

export function readGatewayLogTail(maxLines = 160): Promise<GatewayLogTail> {
  return invokeGateway<GatewayLogTail>("read_gateway_log_tail", { maxLines });
}
