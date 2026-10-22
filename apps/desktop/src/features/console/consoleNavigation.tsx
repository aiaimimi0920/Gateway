import { Crown, KeyRound, Settings, Ticket, Wrench } from "lucide-react";
import type { ShellNavItem } from "../shell/AppShell";

function EntitlementIcon() {
  return <Crown size={17} aria-hidden="true" />;
}

function OperationsIcon() {
  return <Wrench size={17} aria-hidden="true" />;
}

function ModelPoolIcon() {
  return (
    <svg viewBox="0 0 24 24" width="17" height="17" aria-hidden="true">
      <path
        d="M12 3 20 7.5v9L12 21 4 16.5v-9L12 3Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinejoin="round"
      />
      <path
        d="M12 12 12 3M12 12 20 7.5M12 12 20 16.5M12 12 12 21M12 12 4 16.5M12 12 4 7.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.35"
        strokeLinecap="round"
      />
      <circle cx="12" cy="12" r="1.5" fill="currentColor" />
    </svg>
  );
}

export type ConsoleWorkspaceId =
  | "accounts"
  | "groups"
  | "models"
  | "operations"
  | "access"
  | "settings";

/**
 * These workspaces read straight from Postgres/Redis instead of the route
 * draft, so they render even when `routeConfig` has not loaded.
 */
export const ROUTE_CONFIG_INDEPENDENT_WORKSPACES: readonly ConsoleWorkspaceId[] = [
  "operations",
  "access",
  "settings",
];

export function buildConsoleNavigation(
  activeWorkspace: ConsoleWorkspaceId,
  t: (zh: string, en: string) => string,
) {
  const workspaceItems: ShellNavItem<ConsoleWorkspaceId>[] = [
    {
      id: "accounts",
      label: t("凭据池", "Credential pool"),
      icon: <Ticket size={17} aria-hidden="true" />,
    },
    {
      id: "groups",
      label: t("权益组", "Entitlement groups"),
      icon: <EntitlementIcon />,
    },
    {
      id: "models",
      label: t("模型池", "Model pool"),
      icon: <ModelPoolIcon />,
    },
    {
      id: "operations",
      label: t("运维", "Operations"),
      icon: <OperationsIcon />,
    },
    {
      id: "access",
      label: t("密钥", "Keys"),
      icon: <KeyRound size={17} aria-hidden="true" />,
    },
  ];
  const utilityItems: ShellNavItem<ConsoleWorkspaceId>[] = [
    {
      id: "settings",
      label: t("设置", "Settings"),
      icon: <Settings size={17} aria-hidden="true" />,
    },
  ];
  // The board header repeats the active rail label, nothing more.
  const activeWorkspaceLabel =
    [...workspaceItems, ...utilityItems].find((item) => item.id === activeWorkspace)?.label ?? "";
  return { workspaceItems, utilityItems, activeWorkspaceLabel };
}
