import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { areGatewayProfilesEqual } from "../lib/profileState";
import { buildGatewayProfileExportText, parseGatewayProfileTransferText } from "../lib/profileTransfer";
import { createProfileFromTemplate, PROFILE_TEMPLATES } from "../lib/profileTemplates";
import { validateGatewayProfile } from "../lib/profileValidation";
import { checkGatewayProfilePaths, deleteProfile, getGatewayUiRuntimeInfo, listProfiles, loadProfile, saveProfile } from "../lib/tauri";
import type { GatewayDesktopNotice, GatewayProfile, GatewayProfilePathCheck, GatewayUiRuntimeInfo } from "../lib/types";
import { notice, toMessage } from "./desktopNotice";

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

const fallbackRuntimeInfo: GatewayUiRuntimeInfo = {
  appName: "Gateway UI",
  themeFamily: "NeuroTerminal",
  gatewayMode: "headless-first",
};

type GatewayProfileStateOptions = {
  isTauriAvailable: boolean;
  setRuntimeInfo(value: GatewayUiRuntimeInfo): void;
  setStateNotice(value: GatewayDesktopNotice | undefined): void;
  setBusy(value: boolean): void;
  clearRuntimeBoundState(): void;
};

export function useGatewayProfileState({
  isTauriAvailable,
  setRuntimeInfo,
  setStateNotice,
  setBusy,
  clearRuntimeBoundState,
}: GatewayProfileStateOptions) {
  const [profileNames, setProfileNames] = useState<string[]>([]);
  const [selectedProfileName, setSelectedProfileName] = useState(DEFAULT_PROFILE_NAME);
  const [draftProfile, setDraftProfile] = useState<GatewayProfile>(initialProfile);
  const [profileTransferText, setProfileTransferText] = useState("");
  const [importProfileText, setImportProfileText] = useState("");
  const [pathCheck, setPathCheck] = useState<GatewayProfilePathCheck | undefined>();
  const [savedProfileSnapshot, setSavedProfileSnapshot] = useState<GatewayProfile | undefined>(
    initialProfile,
  );
  const selectedProfileNameRef = useRef(selectedProfileName);
  const profileValidation = useMemo(
    () => validateGatewayProfile(draftProfile),
    [draftProfile],
  );
  const canSaveProfile = isTauriAvailable && profileValidation.ok;
  const hasUnsavedProfileChanges = !areGatewayProfilesEqual(draftProfile, savedProfileSnapshot);

  useEffect(() => {
    selectedProfileNameRef.current = selectedProfileName;
  }, [selectedProfileName]);

  const commitSelectedProfileName = useCallback((name: string) => {
    selectedProfileNameRef.current = name;
    setSelectedProfileName(name);
  }, []);

  const clearProfileBoundDerivedState = useCallback(() => {
    setPathCheck(undefined);
    clearRuntimeBoundState();
  }, [clearRuntimeBoundState]);

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

  return {
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
  };
}
