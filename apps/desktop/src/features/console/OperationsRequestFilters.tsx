import { useId, useState } from "react";
import {
  OPERATIONS_DEFAULT_REQUEST_FILTERS,
  type OperationsRequestFilterDraft,
  type OperationsWorkspaceProps,
} from "./operations-contracts";
import "./OperationsRequestFilters.css";

type Props = Pick<
  OperationsWorkspaceProps,
  | "t"
  | "editorLocked"
  | "requests"
  | "requestFilters"
  | "onRequestFiltersChange"
  | "onApplyRequestFilters"
> & {
  patchFilters: (patch: Partial<OperationsRequestFilterDraft>) => void;
};

/** Draft edits stay local until submit; expanding fields never issues a request. */
export function OperationsRequestFilters({
  t,
  editorLocked,
  requests,
  requestFilters,
  onRequestFiltersChange,
  onApplyRequestFilters,
  patchFilters,
}: Props) {
  const statusOptionsId = useId();
  const fields = [
    {
      key: "projectId",
      label: t("项目", "Project"),
      placeholder: t("项目 ID", "Project ID"),
    },
    {
      key: "providerAccountId",
      label: t("服务商账号", "Provider account"),
      placeholder: t("账号 ID", "Account ID"),
    },
    {
      key: "endpointKind",
      label: t("端点类型", "Endpoint kind"),
      placeholder: "chat / embeddings",
    },
    {
      key: "errorCode",
      label: t("错误码", "Error code"),
      placeholder: t("按错误码精确匹配", "Exact error code"),
    },
  ] as const;
  const activeAdvanced = fields.filter(({ key }) =>
    requestFilters[key].trim(),
  ).length;
  // Editing the last populated field must not hide the focused input.
  const [advancedOpen, setAdvancedOpen] = useState(activeAdvanced > 0);
  return (
    <form
      className="nt-operations-filters"
      aria-label={t("请求筛选", "Request filters")}
      onSubmit={(event) => {
        event.preventDefault();
        if (!editorLocked && !requests.loading) onApplyRequestFilters();
      }}
    >
      <div className="nt-operations-filters__primary">
        <label className="nt-field">
          <span>{t("状态", "Status")}</span>
          <input
            className="nt-input"
            list={statusOptionsId}
            placeholder={t("全部状态", "All statuses")}
            value={requestFilters.status}
            onChange={(event) => patchFilters({ status: event.target.value })}
          />
          <datalist id={statusOptionsId}>
            {["completed", "failed", "running", "cancelled"].map((status) => (
              <option key={status} value={status} />
            ))}
          </datalist>
        </label>
        <label className="nt-field">
          <span>{t("条数上限", "Limit")}</span>
          <input
            className="nt-input"
            inputMode="numeric"
            value={requestFilters.limit}
            onChange={(event) => patchFilters({ limit: event.target.value })}
          />
        </label>
        <div className="nt-operations-filters__actions">
          <button
            className="nt-btn nt-btn--primary"
            disabled={editorLocked || requests.loading}
            type="submit"
          >
            {requests.loading
              ? t("查询中…", "Querying…")
              : t("应用筛选", "Apply filters")}
          </button>
          <button
            className="nt-btn nt-btn--outline"
            disabled={editorLocked}
            type="button"
            onClick={() =>
              onRequestFiltersChange({ ...OPERATIONS_DEFAULT_REQUEST_FILTERS })
            }
          >
            {t("重置条件", "Reset")}
          </button>
        </div>
      </div>
      <details
        className="nt-operations-detail nt-operations-filters__advanced"
        open={advancedOpen}
        onToggle={(event) => setAdvancedOpen(event.currentTarget.open)}
      >
        <summary>
          {t("更多筛选", "More filters")}
          {activeAdvanced > 0 ? (
            <span className="nt-badge">{activeAdvanced}</span>
          ) : null}
        </summary>
        <div className="nt-console-field-grid">
          {fields.map(({ key, label, placeholder }) => (
            <label className="nt-field" key={key}>
              <span>{label}</span>
              <input
                className="nt-input"
                value={requestFilters[key]}
                placeholder={placeholder}
                onChange={(event) =>
                  patchFilters({ [key]: event.target.value })
                }
              />
            </label>
          ))}
        </div>
      </details>
      <p className="nt-operations-filters__hint">
        {t(
          "修改或重置条件后，点击“应用筛选”更新结果。",
          "Apply filters after editing or resetting conditions to update results.",
        )}
      </p>
    </form>
  );
}
