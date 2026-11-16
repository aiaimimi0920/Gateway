import { useId, useState } from "react";
import { Badge, Button, Dialog, Field, Input, Panel } from "@neuro/ui";
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
  // Dialog portals leave the form subtree; retain native Enter submission ownership.
  const filterFormId = useId();
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
  const [advancedOpen, setAdvancedOpen] = useState(false);
  return (
    <Panel><form
      id={filterFormId}
      className="nt-operations-filters"
      aria-label={t("请求筛选", "Request filters")}
      onSubmit={(event) => {
        event.preventDefault();
        if (!editorLocked && !requests.loading) onApplyRequestFilters();
      }}
    >
      <div className="nt-operations-filters__primary">
        <Field label={t("状态", "Status")}>
          <Input compact
            list={statusOptionsId}
            placeholder={t("全部状态", "All statuses")}
            value={requestFilters.status}
            onChange={(event) => patchFilters({ status: event.target.value })}
          />
        </Field>
        <datalist id={statusOptionsId}>
          {["completed", "failed", "running", "cancelled"].map((status) => (
            <option key={status} value={status} />
          ))}
        </datalist>
        <Field label={t("条数上限", "Limit")}>
          <Input compact
            inputMode="numeric"
            value={requestFilters.limit}
            onChange={(event) => patchFilters({ limit: event.target.value })}
          />
        </Field>
        <div className="nt-operations-filters__actions">
          <Button compact variant="primary"
            disabled={editorLocked || requests.loading}
            type="submit"
          >
            {requests.loading
              ? t("查询中…", "Querying…")
              : t("应用筛选", "Apply filters")}
          </Button>
          <Button compact
            disabled={editorLocked}
            type="button"
            onClick={() =>
              onRequestFiltersChange({ ...OPERATIONS_DEFAULT_REQUEST_FILTERS })
            }
          >
            {t("重置条件", "Reset")}
          </Button>
        </div>
      </div>
      <Dialog
        className="nt-operations-filters__advanced"
        open={advancedOpen}
        onOpenChange={setAdvancedOpen}
        title={t("更多筛选", "More filters")}
        trigger={<Button compact type="button">{t("更多筛选", "More filters")}
          {activeAdvanced > 0 ? (
            <Badge>{activeAdvanced}</Badge>
          ) : null}
        </Button>}
      >
        <div className="nt-operations-filters__fields">
          {fields.map(({ key, label, placeholder }) => (
            <Field label={label} key={key}>
              <Input compact
                form={filterFormId}
                value={requestFilters[key]}
                placeholder={placeholder}
                onChange={(event) =>
                  patchFilters({ [key]: event.target.value })
                }
              />
            </Field>
          ))}
        </div>
      </Dialog>
    </form></Panel>
  );
}
