import * as Tabs from "@radix-ui/react-tabs";
import { Gauge, ListOrdered, ShieldAlert } from "lucide-react";
import type { Ref } from "react";
import type {
  OperationsWorkspaceProps,
} from "./operations-contracts";
import { databaseOperationsAvailability } from "./operationsAvailability";
import "./OperationsNavigation.css";

type Props = Pick<
  OperationsWorkspaceProps,
  "t" | "operatorSummary" | "incidentSummary"
> & {
  requestsTabRef: Ref<HTMLButtonElement>;
};

/** Navigation describes tasks, not data sources; counts keep their original scope. */
export function OperationsNavigation({
  t,
  operatorSummary,
  incidentSummary,
  requestsTabRef,
}: Props) {
  const availability = databaseOperationsAvailability(operatorSummary);
  const sections = [
    {
      id: "live",
      label: t("运行概览", "Overview"),
      icon: Gauge,
    },
    {
      id: "requests",
      label: t("请求排查", "Request log"),
      icon: ListOrdered,
    },
    {
      id: "anomalies",
      label: t("异常处置", "Incident management"),
      icon: ShieldAlert,
    },
  ] as const;
  return (
    <Tabs.List
      className="nt-operations-navigation"
      aria-label={t("运维视图", "Operations views")}
    >
      {sections.map(({ id, label, icon: Icon }) => (
        <Tabs.Trigger
          className="nt-operations-navigation__tab"
          key={id}
          value={id}
          ref={id === "requests" ? requestsTabRef : undefined}
        >
          <Icon size={17} aria-hidden="true" />
          <span>{label}</span>
          {id === "anomalies" && availability === "available" ? (
            incidentSummary.error ? (
              <span className="nt-badge nt-badge--warning">
                {t("读取失败", "Read failed")}
              </span>
            ) : incidentSummary.data &&
              incidentSummary.data.openIncidents > 0 ? (
              <span className="nt-badge nt-badge--danger">
                {t(
                  `${incidentSummary.data.openIncidents} 未处理`,
                  `${incidentSummary.data.openIncidents} open`,
                )}
              </span>
            ) : null
          ) : null}
        </Tabs.Trigger>
      ))}
    </Tabs.List>
  );
}
