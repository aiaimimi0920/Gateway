import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  fetchGatewayHealth,
  fetchGatewayModels,
  fetchGatewayReady,
  gatewayBaseUrl,
  runGatewayChatCompletionTest,
} from "../lib/http";
import { buildGatewayDiagnosticsReport } from "../lib/diagnostics";
import { buildGatewayOnboardingSteps } from "../lib/onboarding";
import { areGatewayProfilesEqual } from "../lib/profileState";
import {
  buildGatewayProfileExportText,
  parseGatewayProfileTransferText,
} from "../lib/profileTransfer";
import { createProfileFromTemplate, PROFILE_TEMPLATES } from "../lib/profileTemplates";
import { validateGatewayProfile } from "../lib/profileValidation";
import {
  checkGatewayProfilePaths,
  deleteProfile,
  getGatewayProcessSnapshot,
  getGatewayUiRuntimeInfo,
  isTauriRuntime,
  listProfiles,
  loadProfile,
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
  GatewayProfilePathCheck,
  GatewayProcessSnapshot,
  GatewayProfile,
  GatewayReadyResponse,
  GatewayUiRuntimeInfo,
} from "../lib/types";

const DEFAULT_PROFILE_NAME = "local-default";

const initialProfile: GatewayProfile = {
  name: DEFAULT_PROFILE_NAME,
  runtimeRole: "standalone",
  gatewayManagementToken: "",
  port: 4200,
  gatewayRedisUrl: "redis://localhost:6379",
  gatewayDatabaseUrl: "",
  gatewayRoutesFile: "",
  logLevel: "info",
  workingDirectory: "",
  extraEnv: [],
};

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

const fallbackRuntimeInfo: GatewayUiRuntimeInfo = {
  appName: "Neuro Gateway",
  themeFamily: "NeuroTerminal",
  gatewayMode: "headless-first",
};

const initialApiTestInput: GatewayApiTestInput = {
  apiKey: "",
  model: "gpt-5-codex",
  message: "Ping from Neuro Gateway desktop.",
};

function notice(tone: GatewayDesktopNotice["tone"], message: string): GatewayDesktopNotice {
  return { tone, message };
}

function toMessage(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

export function useGatewayDesktopState(): GatewayDesktopState {
  const [runtimeInfo, setRuntimeInfo] = useState<GatewayUiRuntimeInfo | undefined>();
  const [profileNames, setProfileNames] = useState<string[]>([]);
  const [selectedProfileName, setSelectedProfileName] = useState(DEFAULT_PROFILE_NAME);
  const [draftProfile, setDraftProfile] = useState<GatewayProfile>(initialProfile);
  const [processSnapshot, setProcessSnapshot] =
    useState<GatewayProcessSnapshot>(initialProcessSnapshot);
  const [healthProbe, setHealthProbe] = useState<GatewayHttpProbe<GatewayHealthResponse>>();
  const [readyProbe, setReadyProbe] = useState<GatewayHttpProbe<GatewayReadyResponse>>();
  const [modelsProbe, setModelsProbe] = useState<GatewayHttpProbe<GatewayModelListResponse>>();
  const [logTail, setLogTail] = useState<GatewayLogTail>({ path: null, lines: [] });
  const [stateNotice, setStateNotice] = useState<GatewayDesktopNotice | undefined>();
  const [busy, setBusy] = useState(false);
  const [apiTestInput, setApiTestInput] = useState<GatewayApiTestInput>(initialApiTestInput);
  const [apiTestResult, setApiTestResult] = useState<GatewayApiTestResult | undefined>();
  const [diagnosticsReportText, setDiagnosticsReportText] = useState<string | undefined>();
  const [profileTransferText, setProfileTransferText] = useState("");
  const [importProfileText, setImportProfileText] = useState("");
  const [pathCheck, setPathCheck] = useState<GatewayProfilePathCheck | undefined>();
  const [savedProfileSnapshot, setSavedProfileSnapshot] = useState<GatewayProfile | undefined>(
    initialProfile,
  );
  const isTauriAvailable = isTauriRuntime();
  const selectedProfileNameRef = useRef(selectedProfileName);

  const baseUrl = useMemo(
    () => gatewayBaseUrl(processSnapshot.port ?? draftProfile.port),
    [draftProfile.port, processSnapshot.port],
  );
  const profileValidation = useMemo(
    () => validateGatewayProfile(draftProfile),
    [draftProfile],
  );
  const canSaveProfile = isTauriAvailable && profileValidation.ok;
  const canStartGateway = isTauriAvailable && profileValidation.ok && !processSnapshot.running;
  const hasUnsavedProfileChanges = !areGatewayProfilesEqual(draftProfile, savedProfileSnapshot);
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
    selectedProfileNameRef.current = selectedProfileName;
  }, [selectedProfileName]);

  const commitSelectedProfileName = useCallback((name: string) => {
    selectedProfileNameRef.current = name;
    setSelectedProfileName(name);
  }, []);

  const clearProfileBoundDerivedState = useCallback(() => {
    setPathCheck(undefined);
    setApiTestResult(undefined);
    setHealthProbe(undefined);
    setReadyProbe(undefined);
    setModelsProbe(undefined);
  }, []);

  const setDraftProfileFromSource = useCallback(
    (profile: GatewayProfile) => {
      setDraftProfile(profile);
      clearProfileBoundDerivedState();
    },
    [clearProfileBoundDerivedState],
  );

  const updateDraftProfileAndInvalidateDerivedState = useCallback((profile: GatewayProfile) => {
    setDraftProfileFromSource(profile);
  }, [setDraftProfileFromSource]);

  const updateApiTestInputAndInvalidateResult = useCallback((input: GatewayApiTestInput) => {
    setApiTestInput(input);
    setApiTestResult(undefined);
  }, []);

  const reloadProfiles = useCallback(async (preferredProfileName?: string) => {
    if (!isTauriAvailable) {
      setProfileNames([DEFAULT_PROFILE_NAME]);
      setRuntimeInfo(fallbackRuntimeInfo);
      commitSelectedProfileName(DEFAULT_PROFILE_NAME);
      return;
    }

    const [info, names] = await Promise.all([getGatewayUiRuntimeInfo(), listProfiles()]);
    const nextProfileNames = names.length > 0 ? names : [DEFAULT_PROFILE_NAME];
    const explicitPreferredProfileName = preferredProfileName?.trim();
    const currentSelectedProfileName =
      explicitPreferredProfileName && nextProfileNames.includes(explicitPreferredProfileName)
        ? explicitPreferredProfileName
        : selectedProfileNameRef.current;
    const nextSelectedProfileName = nextProfileNames.includes(currentSelectedProfileName)
      ? currentSelectedProfileName
      : nextProfileNames[0];
    setRuntimeInfo(info);
    setProfileNames(nextProfileNames);
    commitSelectedProfileName(nextSelectedProfileName);

    if (names.includes(nextSelectedProfileName)) {
      try {
        const profile = await loadProfile(nextSelectedProfileName);
        setDraftProfileFromSource(profile);
        setSavedProfileSnapshot(profile);
      } catch (error) {
        setStateNotice(notice("warning", `加载 profile 失败：${toMessage(error)}`));
      }
    } else if (nextSelectedProfileName === DEFAULT_PROFILE_NAME) {
      setDraftProfileFromSource(initialProfile);
      setSavedProfileSnapshot(initialProfile);
    }
  }, [commitSelectedProfileName, isTauriAvailable, setDraftProfileFromSource]);

  const selectProfile = useCallback(
    async (name: string) => {
      commitSelectedProfileName(name);
      if (!isTauriAvailable) {
        setDraftProfileFromSource({ ...draftProfile, name });
        return;
      }
      try {
        const profile = await loadProfile(name);
        setDraftProfileFromSource(profile);
        setSavedProfileSnapshot(profile);
      } catch (error) {
        if (name === DEFAULT_PROFILE_NAME) {
          setDraftProfileFromSource(initialProfile);
          setSavedProfileSnapshot(initialProfile);
          return;
        }
        setStateNotice(notice("danger", `读取 profile 失败：${toMessage(error)}`));
      }
    },
    [commitSelectedProfileName, draftProfile, isTauriAvailable, setDraftProfileFromSource],
  );

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

  const saveDraftProfile = useCallback(async () => {
    if (!profileValidation.ok) {
      setStateNotice(notice("warning", `请先修正 profile 配置：${profileValidation.errors[0]}`));
      return;
    }

    setBusy(true);
    try {
      await saveProfile(draftProfile);
      setSavedProfileSnapshot(draftProfile);
      commitSelectedProfileName(draftProfile.name);
      await reloadProfiles(draftProfile.name);
      setStateNotice(notice("success", `已保存 profile：${draftProfile.name}`));
    } catch (error) {
      setStateNotice(notice("danger", `保存 profile 失败：${toMessage(error)}`));
    } finally {
      setBusy(false);
    }
  }, [commitSelectedProfileName, draftProfile, profileValidation, reloadProfiles]);

  const deleteSelectedProfile = useCallback(async () => {
    if (selectedProfileName === DEFAULT_PROFILE_NAME) {
      setDraftProfileFromSource(initialProfile);
      setSavedProfileSnapshot(initialProfile);
      setStateNotice(notice("info", "默认 profile 已重置为本地模板。"));
      return;
    }

    setBusy(true);
    try {
      await deleteProfile(selectedProfileName);
      commitSelectedProfileName(DEFAULT_PROFILE_NAME);
      setDraftProfileFromSource(initialProfile);
      setSavedProfileSnapshot(initialProfile);
      await reloadProfiles(DEFAULT_PROFILE_NAME);
      setStateNotice(notice("success", `已删除 profile：${selectedProfileName}`));
    } catch (error) {
      setStateNotice(notice("danger", `删除 profile 失败：${toMessage(error)}`));
    } finally {
      setBusy(false);
    }
  }, [commitSelectedProfileName, reloadProfiles, selectedProfileName, setDraftProfileFromSource]);

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

  const exportDraftProfile = useCallback(async () => {
    const exportText = buildGatewayProfileExportText(draftProfile);
    setProfileTransferText(exportText);

    if (!navigator.clipboard?.writeText) {
      setStateNotice(notice("warning", "脱敏 profile JSON 已生成，但当前运行时不支持剪贴板写入。"));
      return;
    }

    try {
      await navigator.clipboard.writeText(exportText);
      setStateNotice(notice("success", "已复制脱敏 profile JSON。"));
    } catch (error) {
      setStateNotice(notice("warning", `复制失败；脱敏 profile JSON 已显示在配置面板：${toMessage(error)}`));
    }
  }, [draftProfile]);

  const importDraftProfile = useCallback(async () => {
    try {
      const importedProfile = parseGatewayProfileTransferText(importProfileText);
      const importedValidation = validateGatewayProfile(importedProfile);
      setDraftProfileFromSource(importedProfile);
      commitSelectedProfileName(importedProfile.name);
      if (importedValidation.ok) {
        setStateNotice(notice("success", `已导入到草稿：${importedProfile.name}。请确认后保存。`));
      } else {
        setStateNotice(
          notice("warning", `已导入到草稿，但需要修正配置：${importedValidation.errors[0]}`),
        );
      }
    } catch (error) {
      setStateNotice(notice("danger", `导入 profile 失败：${toMessage(error)}`));
    }
  }, [commitSelectedProfileName, importProfileText, setDraftProfileFromSource]);

  const checkProfilePaths = useCallback(async () => {
    if (!isTauriAvailable) {
      setStateNotice(notice("warning", "浏览器预览模式不能检查本地文件路径。"));
      return;
    }

    try {
      const result = await checkGatewayProfilePaths(draftProfile);
      setPathCheck(result);
      const failedChecks = [
        result.sidecar,
        result.workingDirectory,
        result.gatewayRoutesFile,
        result.redis,
        result.database,
      ].filter((item) => !item.ok);
      if (result.preflightOk) {
        setStateNotice(notice("success", "Profile 依赖预检通过。"));
      } else {
        setStateNotice(notice("warning", `Profile 依赖预检发现 ${failedChecks.length} 个问题。`));
      }
    } catch (error) {
      setStateNotice(notice("danger", `检查 profile 路径失败：${toMessage(error)}`));
    }
  }, [draftProfile, isTauriAvailable]);

  const applyProfileTemplate = useCallback((templateId: string) => {
    try {
      const profileFromTemplate = createProfileFromTemplate(templateId);
      const templateTitle =
        PROFILE_TEMPLATES.find((template) => template.id === templateId)?.title ??
        profileFromTemplate.name;
      setDraftProfileFromSource(profileFromTemplate);
      commitSelectedProfileName(profileFromTemplate.name);
      setStateNotice(
        notice("info", `已应用 profile 模板：${templateTitle}。请确认后保存。`),
      );
    } catch (error) {
      setStateNotice(notice("danger", `应用 profile 模板失败：${toMessage(error)}`));
    }
  }, [commitSelectedProfileName, setDraftProfileFromSource]);

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
