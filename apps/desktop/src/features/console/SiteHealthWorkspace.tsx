import { useMemo, useState } from "react";
import type { GatewayApiClient } from "../../api/client";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { isRecord } from "./routeDocument";
import type { SiteHealthConfig } from "./siteHealth";
import {
  healthSecretIdentity,
  type SiteHealthSecretChange,
  type SiteHealthSecretEdit,
} from "./siteHealthSecretDraft";
import { routeAccountGroupsFromDocument } from "./routeAccountCatalog";
import { useSiteHealthForm } from "./useSiteHealthForm";
import { useSiteHealthTelemetry } from "./useSiteHealthTelemetry";
import { validateSiteHealthPlans } from "./siteHealthValidation";
import { SiteHealthProfileFields } from "./SiteHealthProfileFields";
import { SiteHealthPolicyFields } from "./SiteHealthPolicyFields";
import { SiteHealthAuthFields } from "./SiteHealthAuthFields";
import { SiteHealthPlanLayout } from "./SiteHealthPlanLayout";
import { SiteHealthReport } from "./SiteHealthReport";
import "./SiteHealthWorkspace.css";

type Props = {
  document: ConsoleRouteDocument | null;
  client: GatewayApiClient;
  managementToken: string | null;
  locked: boolean;
  draftDirty: boolean;
  onSave(
    id: string,
    config: SiteHealthConfig | null,
    secrets?: SiteHealthSecretChange[],
  ): void;
  providerId?: string;
  initiallyOpen?: boolean;
  hasSecretAccess?: boolean;
  requestSecretAccess?(): void;
  secretAccess?: { busy: boolean; error: string | null };
  configuredSecrets?: ReadonlySet<string>;
  pendingSecrets?: readonly SiteHealthSecretEdit[];
  t(zh: string, en: string): string;
};

// The dialog is outside the flip card: longer plan forms never resize either face.
export function SiteHealthWorkspace({
  document,
  client,
  managementToken,
  locked,
  draftDirty,
  onSave,
  t,
  providerId: fixedProviderId,
  initiallyOpen = false,
  hasSecretAccess = false,
  requestSecretAccess,
  secretAccess,
  configuredSecrets,
  pendingSecrets,
}: Props) {
  const [open, setOpen] = useState(initiallyOpen);
  const [id, setId] = useState("");
  const [error, setError] = useState<string | null>(null);
  const providers = useMemo(
    () => (document?.providers ?? []).filter(isRecord),
    [document],
  );
  const provider =
    fixedProviderId !== undefined
      ? providers.find((p) => p.id === fixedProviderId)
      : (providers.find((p) => p.id === id) ?? providers[0]);
  const providerId = String(provider?.id ?? "");
  // A dialog already owns disclosure. Native details creates an extra content box
  // that prevents the bounded flex body from scrolling in Chromium.
  const Container = fixedProviderId !== undefined ? "div" : "details";
  // Remount the editor on provider switches, so validation and passwords never cross providers.
  return (
    <Container
      className="nt-panel nt-site-health"
      open={open}
      onToggle={(event) => {
        if (event.currentTarget instanceof HTMLDetailsElement)
          setOpen(event.currentTarget.open);
      }}
    >
      {fixedProviderId === undefined && (
        <summary>{t("站点健康", "Site health")}</summary>
      )}
      {open && (
        <>
          {fixedProviderId === undefined && (
            <label className="nt-field">
              <span>{t("站点", "Site")}</span>
              <select
                className="nt-select"
                value={providerId}
                onChange={(e) => {
                  setId(e.target.value);
                  setError(null);
                }}
              >
                {providers.map((p) => (
                  <option key={String(p.id)} value={String(p.id)}>
                    {String(p.label ?? p.id)}
                  </option>
                ))}
              </select>
            </label>
          )}
          <SiteHealthEditor
            key={providerId}
            {...{
              document,
              client,
              managementToken,
              locked,
              draftDirty,
              onSave,
              t,
              hasSecretAccess,
              requestSecretAccess,
              secretAccess,
              configuredSecrets,
              pendingSecrets,
              provider,
              providerId,
              open,
              error,
              setError,
            }}
          />
        </>
      )}
    </Container>
  );
}

type EditorProps = Props & {
  provider: Record<string, unknown> | undefined;
  providerId: string;
  open: boolean;
  error: string | null;
  setError(error: string | null): void;
};
function SiteHealthEditor({
  document,
  client,
  managementToken,
  locked,
  draftDirty,
  onSave,
  t,
  hasSecretAccess = false,
  requestSecretAccess,
  secretAccess,
  configuredSecrets,
  pendingSecrets,
  provider,
  providerId,
  open,
  error,
  setError,
}: EditorProps) {
  const form = useSiteHealthForm(provider, hasSecretAccess);
  const telemetry = useSiteHealthTelemetry(
    client,
    managementToken,
    providerId,
    form.selectedId,
    open,
  );
  const groups = useMemo(
    () => (document ? routeAccountGroupsFromDocument(document) : []),
    [document],
  );
  const save = () => {
    if (locked || telemetry.busy || !providerId) return;
    try {
      const result = form.build();
      if (result.config)
        validateSiteHealthPlans(
          result.config,
          new Set(groups.map((g) => g.id)),
        );
      if (result.changes.length)
        onSave(providerId, result.config, result.changes);
      else onSave(providerId, result.config);
      setError(null);
    } catch (error) {
      setError(error instanceof Error ? error.message : String(error));
    }
  };
  const configured =
    configuredSecrets?.has(healthSecretIdentity(providerId, form.selectedId)) ??
    false;
  const pending = pendingSecrets?.find(
    (p) => p.providerId === providerId && p.profileId === form.selectedId,
  )?.operation;
  return (
    <SiteHealthPlanLayout
      config={form.config}
      selectedId={form.selectedId}
      groups={groups}
      enabled={form.enabled}
      locked={locked || !providerId}
      onEnable={form.setEnabled}
      onChange={form.changeConfig}
      onSelect={form.select}
      t={t}
      actions={
        <>
          <button
            type="button"
            className="nt-btn nt-btn--outline"
            aria-label={
              telemetry.busy
                ? t("识别中", "Detecting")
                : t("立即识别接口", "Detect interfaces now")
            }
            disabled={
              locked ||
              telemetry.busy ||
              !providerId ||
              draftDirty ||
              form.dirty ||
              !form.enabled ||
              !managementToken
            }
            onClick={telemetry.detect}
          >
            {telemetry.busy
              ? t("识别中", "Detecting")
              : t("立即识别", "Detect now")}
          </button>
          <button
            type="button"
            className="nt-btn nt-btn--primary"
            aria-label={t("保存站点策略", "Save site policy")}
            disabled={locked || telemetry.busy || !providerId}
            onClick={save}
          >
            {t("保存方案", "Save plan")}
          </button>
        </>
      }
      report={
        <>
          {(error || telemetry.error) && (
            <div className="nt-alert nt-alert--danger" role="alert">
              {error || telemetry.error}
            </div>
          )}
          <SiteHealthReport
            report={telemetry.report}
            maxAge={form.policy.max_age_secs}
            edited={form.dirty || draftDirty}
            t={t}
          />
        </>
      }
    >
      <SiteHealthProfileFields
        config={form.config}
        selectedId={form.selectedId}
        groups={groups}
        onChange={form.changeConfig}
        t={t}
      />
      <SiteHealthPolicyFields
        policy={form.policy}
        onChange={form.changePolicy}
        groups={form.text.groups}
        mapping={form.text.mapping}
        onText={(groups, mapping) => form.changeText({ groups, mapping })}
        t={t}
      />
      <SiteHealthAuthFields
        policy={form.policy}
        onChange={form.changePolicy}
        secret={form.secret}
        onSecret={form.changeSecret}
        configured={configured}
        pending={pending}
        hasSecretAccess={hasSecretAccess}
        requestSecretAccess={requestSecretAccess}
        secretAccess={secretAccess}
        t={t}
      />
    </SiteHealthPlanLayout>
  );
}
