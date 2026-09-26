import { Activity, KeyRound, Layers, Settings, ShieldCheck, UsersRound } from "lucide-react";
import type { ShellNavItem } from "../shell/AppShell";

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
      icon: <KeyRound size={17} aria-hidden="true" />,
    },
    {
      id: "groups",
      label: t("权益组", "Entitlement groups"),
      icon: <UsersRound size={17} aria-hidden="true" />,
    },
    {
      id: "models",
      label: t("模型池", "Model pool"),
      icon: <Layers size={17} aria-hidden="true" />,
    },
    {
      id: "operations",
      label: t("运维", "Operations"),
      icon: <Activity size={17} aria-hidden="true" />,
    },
    {
      id: "access",
      label: t("访问密钥", "Access keys"),
      icon: <ShieldCheck size={17} aria-hidden="true" />,
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
