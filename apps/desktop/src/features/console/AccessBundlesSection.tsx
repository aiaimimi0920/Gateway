import { Layers } from "lucide-react";
import { Badge, Button, DataTable, EmptyState, Field, Input, Notice, Panel, type TableColumn } from "@neuro/ui";

import type { ConsoleAccessBundle, ConsoleAccessCatalog } from "../../api/contracts";
import { Section } from "./AccessWorkspacePrimitives";
import {
  ACCESS_DEFAULT_BUNDLE_DRAFT,
  type AccessBundleDraft,
  type AccessPanelState,
  type AccessSectionId,
  type TranslateFn,
} from "./accessKeysTypes";
import { accessStatusTone, formatCount, formatText } from "./accessWorkspaceFormatting";

export type AccessBundlesSectionProps = {
  t: TranslateFn;
  editorLocked: boolean;
  catalog: AccessPanelState<ConsoleAccessCatalog>;
  bundleDraft: AccessBundleDraft;
  onBundleDraftChange: (next: AccessBundleDraft) => void;
  onCreateBundle: () => void;
  creatingBundle: boolean;
  bundleFormIncomplete: boolean;
  bundles: ConsoleAccessBundle[];
  itemCountByBundle: Map<string, number>;
  catalogEmpty: boolean;
  onToggle: (id: AccessSectionId) => void;
  open: boolean;
};

export function AccessBundlesSection({
  t,
  editorLocked,
  catalog,
  bundleDraft,
  onBundleDraftChange,
  onCreateBundle,
  creatingBundle,
  bundleFormIncomplete,
  bundles,
  itemCountByBundle,
  catalogEmpty,
  onToggle,
  open,
}: AccessBundlesSectionProps) {
  const patchBundle = (patch: Partial<AccessBundleDraft>) =>
    onBundleDraftChange({ ...bundleDraft, ...patch });
  const columns: TableColumn<ConsoleAccessBundle>[] = [
    {
      key: "name", header: t("名称", "Name"), rowHeader: true, width: "44%", cell: (bundle) => <div title={bundle.id}>
        {formatText(bundle.displayName)}<small className="nt-access-subtext">{bundle.slug}</small>
      </div>
    },
    { key: "status", header: t("状态", "Status"), cell: (bundle) => <Badge tone={accessStatusTone(bundle.status)}>{formatText(bundle.status)}</Badge> },
    { key: "billing", header: t("计费", "Billing"), cell: (bundle) => formatText(bundle.billingMode) },
    { key: "rows", header: t("售卖行", "Access rows"), cell: (bundle) => formatCount(itemCountByBundle.get(bundle.id) ?? 0) },
  ];

  return (
    <Section
      icon={<Layers size={17} />}
      id="bundles"
      onToggle={onToggle}
      open={open}
      title={t("权益包", "Bundles")}
    >
      <Panel className="nt-access-card">
        <h3>{t("新建权益包", "Create a bundle")}</h3>
        <div className="nt-access-fields">
          <Field label={t("Slug", "Slug")}>
            <Input
              onChange={(event) => patchBundle({ slug: event.target.value })}
              value={bundleDraft.slug}
            />
          </Field>
          <Field label={t("显示名", "Display name")}>
            <Input
              onChange={(event) => patchBundle({ displayName: event.target.value })}
              value={bundleDraft.displayName}
            />
          </Field>
          <Field label={t("项目 ID", "Project id")}>
            <Input
              onChange={(event) => patchBundle({ projectId: event.target.value })}
              placeholder={t("留空表示全局", "Blank means global")}
              value={bundleDraft.projectId}
            />
          </Field>
          <Field label={t("计费模式", "Billing mode")}>
            <Input
              onChange={(event) => patchBundle({ billingMode: event.target.value })}
              placeholder="metered / prepaid"
              value={bundleDraft.billingMode}
            />
          </Field>
          <Field label={t("状态", "Status")}>
            <Input
              onChange={(event) => patchBundle({ status: event.target.value })}
              placeholder="active / disabled"
              value={bundleDraft.status}
            />
          </Field>
          <Field label={t("描述", "Description")}>
            <Input
              onChange={(event) => patchBundle({ description: event.target.value })}
              value={bundleDraft.description}
            />
          </Field>
        </div>
        <div className="nt-access-actions">
          <Button variant="primary" loading={creatingBundle}
            disabled={editorLocked || creatingBundle || bundleFormIncomplete}
            onClick={onCreateBundle}
            type="button"
          >
            {t("创建", "Create")}
          </Button>
          <Button
            disabled={editorLocked}
            onClick={() => onBundleDraftChange({ ...ACCESS_DEFAULT_BUNDLE_DRAFT })}
            type="button"
          >
            {t("重置", "Reset")}
          </Button>
        </div>
      </Panel>

      <Panel className="nt-access-card">
        <h3>{t("权益包台账", "Bundle ledger")}</h3>
        {catalog.loading && catalogEmpty ? (
          <Notice>
            {t("正在读取…", "Loading…")}
          </Notice>
        ) : bundles.length === 0 ? (
          <div role="status">
            <EmptyState title={t("暂无权益包", "No bundles yet")} />
          </div>
        ) : (
          <div className="nt-access-bundle-ledger">
            <DataTable label={t("权益包台账", "Bundle ledger")} rows={bundles} columns={columns} getRowKey={(bundle) => bundle.id} />
          </div>
        )}
      </Panel>
    </Section>
  );
}
