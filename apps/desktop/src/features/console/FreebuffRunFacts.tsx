import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { FreebuffRun } from "./freebuffRun";
const phases = {
  start_pending: ["START 待确认", "START unconfirmed"], started: ["START 已确认", "START confirmed"],
  model_pending: ["模型发送结果未确认", "Model send unconfirmed"], model_observed: ["已观察模型费用", "Model cost observed"],
  step_stopped: ["模型步骤已停止", "Model step stopped"], finish_pending: ["FINISH 待确认", "FINISH unconfirmed"],
  finish_accepted: ["FINISH 已接受（未结算）", "FINISH accepted (not settled)"],
} as const;
export function FreebuffRunFacts({ run }: { run?: FreebuffRun | null }) {
  const { t } = useUiLocale();
  if (!run) return null;
  const phase = phases[run.phase];
  return <div>{t(phase[0], phase[1])} · {t("已报告步骤", "Reported steps")}：{run.totals?.totalSteps ?? "—"}
    {" · "}{t("已报告 SDK credits", "Reported SDK credits")}：{run.totals?.totalCredits ?? "—"}
    {run.totals === null && !!run.priorTotals?.totalSteps && <span>{" · "}{t("此前已报告 SDK credits", "Previously reported SDK credits")}：{run.priorTotals.totalCredits}</span>}</div>;
}
