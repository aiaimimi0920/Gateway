import type { GatewayProfile, GatewayProfileTemplate } from "./types";

export const PROFILE_TEMPLATES: GatewayProfileTemplate[] = [
  {
    id: "local-default",
    title: "本地默认",
    description: "使用 localhost Redis 和默认 Gateway 工作目录，适合首次启动。",
    profile: {
      name: "local-default",
      runtimeRole: "standalone",
      gatewayManagementToken: "",
      port: 4200,
      gatewayRedisUrl: "redis://localhost:6379",
      gatewayDatabaseUrl: "",
      gatewayRoutesFile: "",
      logLevel: "info",
      workingDirectory: "",
      extraEnv: [],
    },
  },
  {
    id: "routes-yaml",
    title: "指定 routes.yaml",
    description: "显式使用 Gateway/routes.yaml，适合已有路由配置的本地用户。",
    profile: {
      name: "local-routes",
      runtimeRole: "standalone",
      gatewayManagementToken: "",
      port: 4200,
      gatewayRedisUrl: "redis://localhost:6379",
      gatewayDatabaseUrl: "",
      gatewayRoutesFile: "routes.yaml",
      logLevel: "info",
      workingDirectory: "",
      extraEnv: [],
    },
  },
  {
    id: "local-debug",
    title: "本地调试",
    description: "设置 RUST_LOG=debug，适合排查启动、路径或 provider 配置问题。",
    profile: {
      name: "local-debug",
      runtimeRole: "standalone",
      gatewayManagementToken: "",
      port: 4201,
      gatewayRedisUrl: "redis://localhost:6379",
      gatewayDatabaseUrl: "",
      gatewayRoutesFile: "routes.yaml",
      logLevel: "debug",
      workingDirectory: "",
      extraEnv: [],
    },
  },
];

export function createProfileFromTemplate(templateId: string): GatewayProfile {
  const template = PROFILE_TEMPLATES.find((item) => item.id === templateId);
  if (!template) {
    throw new Error(`Unknown Gateway profile template: ${templateId}`);
  }
  return {
    ...template.profile,
    extraEnv: template.profile.extraEnv.map((entry) => ({ ...entry })),
  };
}
