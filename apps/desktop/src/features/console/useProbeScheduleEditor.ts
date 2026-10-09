import { useCallback, type Dispatch, type SetStateAction } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { pushAppToast } from "../../components/AppToast";
import type { AccountsLedgerPilotAccount, AccountsLedgerPilotSection } from "./AccountsLedgerWorkspace";
import type { PilotActionDialogState } from "./PilotActionDialog";
import { updateCredentialProbeSchedule, updateProviderProbeSchedule } from "./credentialDocument";
import { optionalBoolean } from "./pilotPoolPolicy";
import { parseRouteDocument, isRecord } from "./routeDocument";
import { useTestPolicyEditor } from "./useTestPolicyEditor";

type ProbeScheduleEditorOptions = {
  editorText: string;
  pilotActionDialog: PilotActionDialogState | null;
  setPilotActionDialog: Dispatch<SetStateAction<PilotActionDialogState | null>>;
  pilotScheduleEnabled: boolean;
  setPilotScheduleEnabled: Dispatch<SetStateAction<boolean>>;
  pilotScheduleIntervalMinutes: string;
  setPilotScheduleIntervalMinutes: Dispatch<SetStateAction<string>>;
  setError: Dispatch<SetStateAction<string | null>>;
  replaceEditorDocument: (document: ConsoleRouteDocument, syncStructuredEditors?: boolean) => void;
  t: (zh: string, en: string) => string;
};

export function useProbeScheduleEditor({
  editorText,
  pilotActionDialog,
  setPilotActionDialog,
  pilotScheduleEnabled,
  setPilotScheduleEnabled,
  pilotScheduleIntervalMinutes,
  setPilotScheduleIntervalMinutes,
  setError,
  replaceEditorDocument,
  t,
}: ProbeScheduleEditorOptions) {
  const testPolicyEditor = useTestPolicyEditor(editorText, replaceEditorDocument, setError, t);
  const openProviderScheduleDialog = useCallback(
    (section: AccountsLedgerPilotSection) => {
      let document: ConsoleRouteDocument;
      try {
        document = parseRouteDocument(editorText);
      } catch {
        setError(t("当前 JSON 草稿不可解析。", "The current JSON draft is invalid."));
        return;
      }
      const provider = document.providers.find(
        (entry): entry is Record<string, unknown> =>
          isRecord(entry) && entry.id === section.providerId,
      );
      if (!provider) {
        setError(
          t(
            `服务商 ${section.providerLabel} 不在当前路由草稿中。`,
            `Provider ${section.providerLabel} is not present in the current route draft.`,
          ),
        );
        return;
      }
      const credentials = Array.isArray(provider.credentials)
        ? provider.credentials.filter((entry): entry is Record<string, unknown> => isRecord(entry))
        : [];
      const scheduleTargets = credentials.length > 0 ? credentials : [provider];
      const enabled =
        scheduleTargets.length > 0 &&
        scheduleTargets.every(
          (target) => optionalBoolean(target, "scheduled_probe_enabled") === true,
        );
      const intervals = scheduleTargets
        .map((target) => target.scheduled_probe_interval_minutes)
        .filter(
          (value): value is number =>
            typeof value === "number" && Number.isInteger(value) && value >= 1,
        );
      const interval =
        intervals.length > 0 && intervals.every((value) => value === intervals[0])
          ? intervals[0]
          : 60;
      setPilotScheduleEnabled(enabled);
      setPilotScheduleIntervalMinutes(String(interval));
      setPilotActionDialog({ kind: "provider-schedule", section });
      setError(null);
    },
    [editorText, t],
  );

  const openPilotStatsDialog = useCallback(
    (providerId: string, account: AccountsLedgerPilotAccount) => {
      setPilotActionDialog({
        kind: "stats",
        providerId,
        account,
      });
      setPilotScheduleEnabled(account.scheduledProbeEnabled ?? false);
      setPilotScheduleIntervalMinutes(String(account.scheduledProbeIntervalMinutes ?? 60));
    },
    [],
  );

  const applyPilotProbeSchedule = useCallback(() => {
    if (pilotActionDialog?.kind !== "stats") {
      return;
    }
    const intervalMinutes = Number(pilotScheduleIntervalMinutes);
    if (
      !Number.isInteger(intervalMinutes) ||
      intervalMinutes < 1 ||
      intervalMinutes > 10_080
    ) {
      setError(
        t(
          "定时测试间隔必须是 1 到 10080 分钟之间的整数。",
          "The scheduled-test interval must be an integer between 1 and 10080 minutes.",
        ),
      );
      return;
    }
    let document: ConsoleRouteDocument;
    try {
      document = parseRouteDocument(editorText);
      const nextDocument = updateCredentialProbeSchedule(document, {
        providerId: pilotActionDialog.providerId,
        credentialId: pilotActionDialog.account.accountId,
        mode: pilotActionDialog.account.mode,
        enabled: pilotScheduleEnabled,
        intervalMinutes,
      });
      replaceEditorDocument(nextDocument, true);
      setError(null);
      pushAppToast(
        "info",
        pilotScheduleEnabled
          ? t(
              `定时测试已设为每 ${intervalMinutes} 分钟执行，保存路由配置后生效。`,
              `Scheduled testing will run every ${intervalMinutes} minutes after the route config is saved.`,
            )
          : t(
              "定时测试已在草稿中关闭，保存路由配置后生效。",
              "Scheduled testing is disabled in the draft and will take effect after saving.",
            ),
      );
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [
    editorText,
    pilotActionDialog,
    pilotScheduleEnabled,
    pilotScheduleIntervalMinutes,
    replaceEditorDocument,
    t,
  ]);

  const applyProviderProbeSchedule = useCallback(() => {
    if (pilotActionDialog?.kind !== "provider-schedule") {
      return;
    }
    const intervalMinutes = Number(pilotScheduleIntervalMinutes);
    if (
      !Number.isInteger(intervalMinutes) ||
      intervalMinutes < 1 ||
      intervalMinutes > 10_080
    ) {
      setError(
        t(
          "自动定时测试间隔必须是 1 到 10080 分钟之间的整数。",
          "The automatic test interval must be an integer between 1 and 10080 minutes.",
        ),
      );
      return;
    }
    try {
      const document = parseRouteDocument(editorText);
      const nextDocument = updateProviderProbeSchedule(document, {
        providerId: pilotActionDialog.section.providerId,
        enabled: pilotScheduleEnabled,
        intervalMinutes,
      });
      replaceEditorDocument(nextDocument, true);
      setError(null);
      pushAppToast(
        "info",
        pilotScheduleEnabled
          ? t(
              `${pilotActionDialog.section.providerLabel} 的全部账号将每 ${intervalMinutes} 分钟执行一次单点测试，保存路由配置后生效。`,
              `Every ${pilotActionDialog.section.providerLabel} account will run a single-point test every ${intervalMinutes} minutes after the route config is saved.`,
            )
          : t(
              `${pilotActionDialog.section.providerLabel} 的自动定时测试已在草稿中关闭，保存路由配置后生效。`,
              `Automatic scheduled tests for ${pilotActionDialog.section.providerLabel} are disabled in the draft and will take effect after saving.`,
            ),
      );
      setPilotActionDialog(null);
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [
    editorText,
    pilotActionDialog,
    pilotScheduleEnabled,
    pilotScheduleIntervalMinutes,
    replaceEditorDocument,
    t,
  ]);

  return { ...testPolicyEditor, openProviderScheduleDialog, openPilotStatsDialog, applyPilotProbeSchedule, applyProviderProbeSchedule };
}
