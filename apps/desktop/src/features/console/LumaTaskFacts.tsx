import { CopyButton, Field, Input } from "@neuro/ui";
import type { LumaTask } from "../../api/console/luma-task";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

/** Supplier state and credits are independent of local request success or customer billing. */
export function LumaTaskFacts({ task }: { task: LumaTask & { mediaVerified?: boolean } }) {
  const { t } = useUiLocale();
  const states = { queued: t("排队中", "Queued"), processing: t("处理中", "Processing"), completed: t("已完成", "Completed"), failed: t("失败", "Failed") };
  return <div className="n-stack">
    <Field label={t("Luma 任务 ID", "Luma task ID")}><Input readOnly value={task.id} /></Field>
    <CopyButton compact label={t("复制 Luma 任务 ID", "Copy Luma task ID")} onCopy={() => navigator.clipboard.writeText(task.id)} />
    <dl className="nt-request-inline-fields">{[
      [t("供应商任务状态", "Supplier task status"), states[task.state]],
      [t("任务模型", "Task model"), task.model],
      [t("任务类型", "Task type"), task.type],
      [t("供应商实际扣费（credits）", "Supplier charge (credits)"), task.usage.credits_charged === null ? t("未知", "Unknown") : String(task.usage.credits_charged)],
      ...(task.mediaVerified === undefined ? [] : [[t("本地媒体验证", "Local media verification"), task.mediaVerified ? t("已通过", "Verified") : t("未验证", "Not verified")]]),
    ].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
  </div>;
}
