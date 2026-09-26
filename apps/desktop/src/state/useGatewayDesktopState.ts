import { useCallback, useEffect, useMemo, useState } from "react";
import {
  fetchGatewayHealth,
  fetchGatewayModels,
  fetchGatewayReady,
  gatewayBaseUrl,
  runGatewayChatCompletionTest,
} from "../lib/http";
import { buildGatewayDiagnosticsReport } from "../lib/diagnostics";
import { buildGatewayOnboardingSteps } from "../lib/onboarding";
import {
  getGatewayProcessSnapshot,
  isTauriRuntime,
  openGatewayLogDirectory,
  readGatewayLogTail,
  saveProfile,
  startGatewaySidecar,
  stopGatewaySidecar,
} from "../lib/tauri";
import type {
  GatewayApiTestInput,
  GatewayApiTestResult,
  GatewayDesktopNotice,
  GatewayDesktopState,
  GatewayHealthResponse,
  GatewayHttpProbe,
  GatewayLogTail,
  GatewayModelListResponse,
  GatewayProcessSnapshot,
  GatewayReadyResponse,
  GatewayUiRuntimeInfo,
} from "../lib/types";
import { notice, toMessage } from "./desktopNotice";
import { useGatewayProfileState } from "./useGatewayProfileState";

const initialProcessSnapshot: GatewayProcessSnapshot = {
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

const initialApiTestInput: GatewayApiTestInput = {
  apiKey: "",
  model: "gpt-5-codex",
  message: "Ping from Neuro Gateway desktop.",
};

export function useGatewayDesktopState(): GatewayDesktopState {
  const [runtimeInfo, setRuntimeInfo] = useState<GatewayUiRuntimeInfo | undefined>();
  const [processSnapshot, setProcessSnapshot] =
    useState<GatewayProcessSnapshot>(initialProcessSnapshot);
  const [healthProbe, setHealthProbe] = useState<GatewayHttpProbe<GatewayHealthResponse>>();
  const [readyProbe, setReadyProbe] = useState<GatewayHttpProbe<GatewayReadyResponse>>();
  const [modelsProbe, setModelsProbe] = useState<GatewayHttpProbe<GatewayModelListResponse>>();
  const [logTail, setLogTail] = useState<GatewayLogTail>({ path: null, lines: [] });
  const [stateNotice, setStateNotice] = useState<GatewayDesktopNotice | undefined>();
  const [busy, setBusy] = useState(false);
  const [autoStartAttempted, setAutoStartAttempted] = useState(false);
  const [apiTestInput, setApiTestInput] = useState<GatewayApiTestInput>(initialApiTestInput);
  const [apiTestResult, setApiTestResult] = useState<GatewayApiTestResult | undefined>();
  const [diagnosticsReportText, setDiagnosticsReportText] = useState<string | undefined>();
  const isTauriAvailable = isTauriRuntime();
  const clearRuntimeBoundState = useCallback(() => {
    setApiTestResult(undefined);
    setHealthProbe(undefined);
    setReadyProbe(undefined);
    setModelsProbe(undefined);
  }, []);
  const {
    profileNames,
    selectedProfileName,
    draftProfile,
    profileTransferText,
    importProfileText,
    pathCheck,
    profileValidation,
    canSaveProfile,
    hasUnsavedProfileChanges,
    commitSelectedProfileName,
    setSavedProfileSnapshot,
    reloadProfiles,
    selectProfile,
    updateDraftProfileAndInvalidateDerivedState,
    saveDraftProfile,
    deleteSelectedProfile,
    exportDraftProfile,
    importDraftProfile,
    setImportProfileText,
    checkProfilePaths,
    applyProfileTemplate,
  } = useGatewayProfileState({
    isTauriAvailable,
    setRuntimeInfo,
    setStateNotice,
    setBusy,
    clearRuntimeBoundState,
  });

  const baseUrl = useMemo(
    () => gatewayBaseUrl(processSnapshot.port ?? draftProfile.port),
    [draftProfile.port, processSnapshot.port],
  );
  const canStartGateway = isTauriAvailable && profileValidation.ok && !processSnapshot.running;
  const startWillSaveDraftProfile = hasUnsavedProfileChanges && !processSnapshot.running;
  const onboardingSteps = useMemo(
    () =>
      buildGatewayOnboardingSteps({
        draftProfile,
        profileNames,
        profileValidation,
        pathCheck,
        processSnapshot,
        healthProbe,
        readyProbe,
        apiTestResult,
        hasUnsavedProfileChanges,
      }),
    [
      apiTestResult,
      draftProfile,
      healthProbe,
      pathCheck,
      processSnapshot,
      profileNames,
      profileValidation,
      readyProbe,
      hasUnsavedProfileChanges,
    ],
  );

  useEffect(() => {
    setAutoStartAttempted(false);
  }, [selectedProfileName]);

  const updateApiTestInputAndInvalidateResult = useCallback((input: GatewayApiTestInput) => {
    setApiTestInput(input);
    setApiTestResult(undefined);
  }, []);

  const refreshLogs = useCallback(async () => {
    if (!isTauriAvailable) {
      setLogTail({
        path: null,
        lines: ["[desktop] 当前在浏览器预览模式，日志需要在 Tauri 桌面运行时中读取。"],
      });
      return;
    }

    try {
      setLogTail(await readGatewayLogTail(180));
    } catch (error) {
      setLogTail({
        path: null,
        lines: [`[desktop] 读取日志失败：${toMessage(error)}`],
      });
    }
  }, [isTauriAvailable]);

  const refreshRuntimeSignals = useCallback(async () => {
    let nextPort: number | null | undefined = null;
    if (isTauriAvailable) {
      try {
        const nextSnapshot = await getGatewayProcessSnapshot();
        nextPort = nextSnapshot.port;
        setProcessSnapshot(nextSnapshot);
      } catch (error) {
        setStateNotice(notice("warning", `读取进程状态失败：${toMessage(error)}`));
      }
    }

    const nextBaseUrl = gatewayBaseUrl(nextPort ?? draftProfile.port);
    const [health, ready] = await Promise.all([
      fetchGatewayHealth(nextBaseUrl),
      fetchGatewayReady(nextBaseUrl),
    ]);
    setHealthProbe(health);
    setReadyProbe(ready);
    await refreshLogs();
  }, [draftProfile.port, isTauriAvailable, refreshLogs]);

  const refreshModels = useCallback(async () => {
    const models = await fetchGatewayModels(baseUrl, apiTestInput.apiKey);
    setModelsProbe(models);
  }, [apiTestInput.apiKey, baseUrl]);

  const refreshAll = useCallback(async () => {
    await Promise.all([reloadProfiles(), refreshRuntimeSignals(), refreshModels()]);
  }, [refreshModels, refreshRuntimeSignals, reloadProfiles]);

  const startGateway = useCallback(async () => {
    if (!profileValidation.ok) {
      setStateNotice(notice("warning", `请先修正 profile 配置：${profileValidation.errors[0]}`));
      return;
    }

    setBusy(true);
    try {
      await saveProfile(draftProfile);
      setSavedProfileSnapshot(draftProfile);
      const snapshot = await startGatewaySidecar(draftProfile.name);
      setProcessSnapshot(snapshot);
      if (snapshot.recentLogLines.length > 0) {
        setLogTail({ path: snapshot.logPath ?? null, lines: snapshot.recentLogLines });
      }
      commitSelectedProfileName(draftProfile.name);
      await reloadProfiles(draftProfile.name);
      if (snapshot.running && snapshot.startupState === "ready") {
        setStateNotice(notice("success", `Gateway 已启动并 ready：${draftProfile.name}`));
      } else if (snapshot.running) {
        setStateNotice(
          notice(
            "warning",
            `Gateway 已启动，但启动探测状态为 ${snapshot.startupState ?? "unknown"}：${
              snapshot.lastError ?? "等待后续健康检查"
            }`,
          ),
        );
      } else {
        setStateNotice(
          notice("danger", `Gateway 启动后退出：${snapshot.lastError ?? "请查看最近日志"}`),
        );
      }
      await refreshRuntimeSignals();
    } catch (error) {
      setStateNotice(notice("danger", `启动 Gateway 失败：${toMessage(error)}`));
    } finally {
      setBusy(false);
    }
  }, [commitSelectedProfileName, draftProfile, profileValidation, refreshRuntimeSignals, reloadProfiles]);

  const stopGateway = useCallback(async () => {
    setBusy(true);
    try {
      const snapshot = await stopGatewaySidecar();
      setProcessSnapshot(snapshot);
      if (snapshot.recentLogLines.length > 0) {
        setLogTail({ path: snapshot.logPath ?? null, lines: snapshot.recentLogLines });
      }
      if (snapshot.shutdownState === "forced") {
        setStateNotice(
          notice("warning", snapshot.lastError ?? "Gateway sidecar 已在 drain 超时后强制终止。"),
        );
      } else if (snapshot.shutdownState === "graceful") {
        setStateNotice(notice("success", "Gateway sidecar 已完成授权 drain 并优雅停止。"));
      } else {
        setStateNotice(notice("info", "Gateway sidecar 已停止。"));
      }
      await refreshRuntimeSignals();
    } catch (error) {
      setStateNotice(notice("danger", `停止 Gateway 失败：${toMessage(error)}`));
    } finally {
      setBusy(false);
    }
  }, [refreshRuntimeSignals]);

  const runApiTest = useCallback(async () => {
    setBusy(true);
    try {
      const result = await runGatewayChatCompletionTest(baseUrl, apiTestInput, {
        requestedAt: new Date().toISOString(),
        profileName: draftProfile.name,
        baseUrl,
        model: apiTestInput.model.trim(),
      });
      setApiTestResult(result);
      setStateNotice(
        notice(
          result.ok ? "success" : "warning",
          result.ok
            ? "API 测试请求已成功返回。"
            : `API 测试请求返回异常：HTTP ${result.status || "network"}`,
        ),
      );
    } finally {
      setBusy(false);
    }
  }, [apiTestInput, baseUrl, draftProfile.name]);

  const openLogDirectory = useCallback(async () => {
    if (!isTauriAvailable) {
      setStateNotice(notice("warning", "浏览器预览模式不能打开本地日志目录。"));
      return;
    }

    try {
      const path = await openGatewayLogDirectory();
      setStateNotice(notice("success", `已打开 Gateway 日志目录：${path}`));
    } catch (error) {
      setStateNotice(notice("danger", `打开日志目录失败：${toMessage(error)}`));
    }
  }, [isTauriAvailable]);

  const copyCurrentLogPath = useCallback(async () => {
    const currentLogPath = logTail.path ?? processSnapshot.logPath ?? null;
    if (!currentLogPath) {
      setStateNotice(notice("warning", "当前没有可复制的 Gateway 日志路径。"));
      return;
    }
    if (!navigator.clipboard?.writeText) {
      setStateNotice(notice("warning", "当前运行时不支持剪贴板写入；日志路径已显示在日志面板中。"));
      return;
    }

    try {
      await navigator.clipboard.writeText(currentLogPath);
      setStateNotice(notice("success", "已复制当前 Gateway 日志路径。"));
    } catch (error) {
      setStateNotice(notice("danger", `复制日志路径失败：${toMessage(error)}`));
    }
  }, [logTail.path, processSnapshot.logPath]);

  const copyDiagnostics = useCallback(async () => {
    const report = buildGatewayDiagnosticsReport({
      runtimeInfo,
      selectedProfileName,
      draftProfile,
      processSnapshot,
      healthProbe,
      readyProbe,
      modelsProbe,
      apiTestResult,
      logTail,
      baseUrl,
      isTauriAvailable,
    });
    setDiagnosticsReportText(report);

    if (!navigator.clipboard?.writeText) {
      setStateNotice(notice("warning", "诊断信息已生成在日志面板，但当前运行时不支持剪贴板写入。"));
      return;
    }

    try {
      await navigator.clipboard.writeText(report);
      setStateNotice(notice("success", "已复制脱敏诊断信息，可粘贴给开发者排查。"));
    } catch (error) {
      setStateNotice(
        notice("danger", `复制诊断信息失败；诊断信息已生成在日志面板：${toMessage(error)}`),
      );
    }
  }, [
    baseUrl,
    apiTestResult,
    draftProfile,
    healthProbe,
    isTauriAvailable,
    logTail,
    modelsProbe,
    processSnapshot,
    readyProbe,
    runtimeInfo,
    selectedProfileName,
  ]);

  useEffect(() => {
    void reloadProfiles();
  }, [reloadProfiles]);

  useEffect(() => {
    void refreshRuntimeSignals();
    const timer = window.setInterval(() => {
      void refreshRuntimeSignals();
    }, 5000);
    return () => window.clearInterval(timer);
  }, [refreshRuntimeSignals]);

  useEffect(() => {
    const backendHealthMissing = healthProbe ? !healthProbe.ok : false;
    if (
      !isTauriAvailable ||
      autoStartAttempted ||
      busy ||
      processSnapshot.running ||
      !canStartGateway ||
      !backendHealthMissing
    ) {
      return;
    }

    setAutoStartAttempted(true);
    setStateNotice(notice("info", "未检测到 Gateway 后端，正在自动启动 gateway.exe。"));
    void startGateway();
  }, [
    autoStartAttempted,
    busy,
    canStartGateway,
    healthProbe,
    isTauriAvailable,
    processSnapshot.running,
    startGateway,
  ]);

  return {
    runtimeInfo,
    profileNames,
    selectedProfileName,
    draftProfile,
    processSnapshot,
    healthProbe,
    readyProbe,
    modelsProbe,
    logTail,
    diagnosticsReportText,
    profileTransferText,
    importProfileText,
    pathCheck,
    onboardingSteps,
    hasUnsavedProfileChanges,
    startWillSaveDraftProfile,
    notice: stateNotice,
    busy,
    apiTestInput,
    apiTestResult,
    profileValidation,
    canSaveProfile,
    canStartGateway,
    baseUrl,
    isTauriAvailable,
    refreshAll,
    reloadProfiles,
    selectProfile,
    updateDraftProfile: updateDraftProfileAndInvalidateDerivedState,
    saveDraftProfile,
    deleteSelectedProfile,
    startGateway,
    stopGateway,
    refreshRuntimeSignals,
    refreshModels,
    refreshLogs,
    openLogDirectory,
    copyCurrentLogPath,
    copyDiagnostics,
    exportDraftProfile,
    importDraftProfile,
    updateImportProfileText: setImportProfileText,
    checkProfilePaths,
    applyProfileTemplate,
    updateApiTestInput: updateApiTestInputAndInvalidateResult,
    runApiTest,
    clearNotice: () => setStateNotice(undefined),
  };
}
