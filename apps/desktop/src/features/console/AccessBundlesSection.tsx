import { Layers } from "lucide-react";

import type { ConsoleAccessBundle, ConsoleAccessCatalog } from "../../api/contracts";
import { Section } from "./AccessWorkspacePrimitives";
import {
  ACCESS_DEFAULT_BUNDLE_DRAFT,
  type AccessBundleDraft,
  type AccessPanelState,
  type AccessSectionId,
  type TranslateFn,
} from "./accessKeysTypes";
import { formatCount, formatText, statusBadgeClass } from "./accessWorkspaceFormatting";

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

  return (
    <Section
      icon={<Layers size={17} />}
      id="bundles"
      onToggle={onToggle}
      open={open}
      title={t("权益包", "Bundles")}
    >
      <article className="nt-pilot-metric-card">
        <h3>{t("新建权益包", "Create a bundle")}</h3>
        <div className="nt-console-field-grid">
          <label className="nt-field">
            <span>{t("Slug", "Slug")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchBundle({ slug: event.target.value })}
              value={bundleDraft.slug}
            />
          </label>
          <label className="nt-field">
            <span>{t("显示名", "Display name")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchBundle({ displayName: event.target.value })}
              value={bundleDraft.displayName}
            />
          </label>
          <label className="nt-field">
            <span>{t("项目 ID", "Project id")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchBundle({ projectId: event.target.value })}
              placeholder={t("留空表示全局", "Blank means global")}
              value={bundleDraft.projectId}
            />
          </label>
          <label className="nt-field">
            <span>{t("计费模式", "Billing mode")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchBundle({ billingMode: event.target.value })}
              placeholder="metered / prepaid"
              value={bundleDraft.billingMode}
            />
          </label>
          <label className="nt-field">
            <span>{t("状态", "Status")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchBundle({ status: event.target.value })}
              placeholder="active / disabled"
              value={bundleDraft.status}
            />
          </label>
          <label className="nt-field">
            <span>{t("描述", "Description")}</span>
            <input
              className="nt-input"
              onChange={(event) => patchBundle({ description: event.target.value })}
              value={bundleDraft.description}
            />
          </label>
        </div>
        <div className="nt-console-form-actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={editorLocked || creatingBundle || bundleFormIncomplete}
            onClick={onCreateBundle}
            type="button"
          >
            {creatingBundle ? t("创建中…", "Creating…") : t("创建", "Create")}
          </button>
          <button
            className="nt-btn nt-btn--secondary"
            disabled={editorLocked}
            onClick={() => onBundleDraftChange({ ...ACCESS_DEFAULT_BUNDLE_DRAFT })}
            type="button"
          >
            {t("重置", "Reset")}
          </button>
        </div>
      </article>

      <article className="nt-pilot-metric-card">
        <h3>{t("权益包台账", "Bundle ledger")}</h3>
        {catalog.loading && catalogEmpty ? (
          <div className="nt-pilot-stats-state" role="status">
            {t("正在读取…", "Loading…")}
          </div>
        ) : bundles.length === 0 ? (
          <div className="nt-pilot-stats-state" role="status">
            {t("暂无权益包", "No bundles yet")}
          </div>
        ) : (
          <div className="nt-table nt-table--bundles">
            <div className="nt-table__head">
              <span>{t("名称", "Name")}</span>
              <span>{t("状态", "Status")}</span>
              <span>{t("计费", "Billing")}</span>
              <span>{t("售卖行", "Access rows")}</span>
            </div>
            {bundles.map((bundle) => (
              <div className="nt-table__row" key={bundle.id}>
                <span title={bundle.id}>
                  {formatText(bundle.displayName)}
                  <small className="nt-console-subtext">{bundle.slug}</small>
                </span>
                <span>
                  <span className={statusBadgeClass(bundle.status)}>
                    {formatText(bundle.status)}
                  </span>
                </span>
                <span>{formatText(bundle.billingMode)}</span>
                <span>{formatCount(itemCountByBundle.get(bundle.id) ?? 0)}</span>
              </div>
            ))}
          </div>
        )}
      </article>
    </Section>
  );
}
