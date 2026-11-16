import { useMemo, useState } from "react";
import { Button, Disclosure, Field, Notice, Select } from "@neuro/ui";
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
  // Remount the editor on provider switches, so validation and passwords never cross providers.
  const content = open && (
    <>
      {fixedProviderId === undefined && (
        <Field label={t("站点", "Site")}>
          <Select
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
          </Select>
        </Field>
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
  );
  // A modal already owns disclosure; keep its flex body free of an extra details box.
  return fixedProviderId !== undefined ? <div className="nt-site-health">{content}</div> : (
    <Disclosure className="nt-site-health" title={t("站点健康", "Site health")} open={open} onOpenChange={setOpen}>
      {content}
    </Disclosure>
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
          <Button wrap
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
          </Button>
          <Button wrap variant="primary"
            aria-label={t("保存站点策略", "Save site policy")}
            disabled={locked || telemetry.busy || !providerId}
            onClick={save}
          >
            {t("保存方案", "Save plan")}
          </Button>
        </>
      }
      report={
        <>
          {(error || telemetry.error) && (
            <Notice tone="danger">
              {error || telemetry.error}
            </Notice>
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
        disabled={locked || !providerId || !form.enabled}
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
