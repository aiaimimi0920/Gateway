import type {
  GatewayApiTestResult,
  GatewayHealthResponse,
  GatewayHttpProbe,
  GatewayOnboardingStep,
  GatewayProfile,
  GatewayProfilePathCheck,
  GatewayProfileValidation,
  GatewayReadyResponse,
  GatewayProcessSnapshot,
} from "./types";

type GatewayOnboardingInput = {
  draftProfile: GatewayProfile;
  profileNames: string[];
  profileValidation: GatewayProfileValidation;
  pathCheck?: GatewayProfilePathCheck;
  processSnapshot: GatewayProcessSnapshot;
  healthProbe?: GatewayHttpProbe<GatewayHealthResponse>;
  readyProbe?: GatewayHttpProbe<GatewayReadyResponse>;
  apiTestResult?: GatewayApiTestResult;
  hasUnsavedProfileChanges: boolean;
};

function statusFromBoolean(ok: boolean, warning = false): GatewayOnboardingStep["status"] {
  if (ok) {
    return "done";
  }
  return warning ? "warning" : "pending";
}

function pathCheckStatus(pathCheck?: GatewayProfilePathCheck): GatewayOnboardingStep["status"] {
  if (!pathCheck) {
    return "pending";
  }
  return pathCheck.preflightOk ? "done" : "warning";
}

export function buildGatewayOnboardingSteps(input: GatewayOnboardingInput): GatewayOnboardingStep[] {
  const profileSaved =
    input.profileNames.includes(input.draftProfile.name) && !input.hasUnsavedProfileChanges;
  const runtimeReady = Boolean(input.readyProbe?.ok || input.healthProbe?.ok);

  return [
    {
      id: "profile-config",
      title: "确认 profile 配置",
      description: "选择 runtime role，并填写端口、Redis、管理令牌、routes 和日志级别。",
      status: statusFromBoolean(input.profileValidation.ok, true),
    },
    {
      id: "path-check",
      title: "运行依赖预检",
      description: "确认 sidecar、Working directory、routes、Redis 与可选 PostgreSQL 可用。",
      status: pathCheckStatus(input.pathCheck),
    },
    {
      id: "save-profile",
      title: "保存 profile",
      description: "显式保存后，桌面端才能用该 profile 启动 sidecar。",
      status: statusFromBoolean(profileSaved),
    },
    {
      id: "start-sidecar",
      title: "启动 Gateway sidecar",
      description: "桌面端只启动同一个 headless gateway.exe。",
      status: statusFromBoolean(input.processSnapshot.running),
    },
    {
      id: "ready-health",
      title: "确认 health / ready",
      description: "等待 /healthz 或 /readyz 返回成功，再进行 API 调用测试。",
      status: statusFromBoolean(runtimeReady),
    },
    {
      id: "api-test",
      title: "运行 API Test",
      description: "使用当前 base URL 发起最小 chat completions 请求。",
      status: statusFromBoolean(Boolean(input.apiTestResult?.ok)),
    },
  ];
}
