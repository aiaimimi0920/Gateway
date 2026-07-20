import type { GatewayProfile, GatewayProfileValidation } from "./types";

const ENV_KEY_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

const RESERVED_ENV_KEYS = new Set([
  "PORT",
  "GATEWAY_RUNTIME_ROLE",
  "GATEWAY_MANAGEMENT_TOKEN",
  "GATEWAY_REDIS_URL",
  "GATEWAY_DATABASE_URL",
  "DATABASE_URL",
  "GATEWAY_ROUTES_FILE",
  "RUST_LOG",
]);

function pushFieldError(
  validation: GatewayProfileValidation,
  field: keyof GatewayProfileValidation["fieldErrors"],
  message: string,
) {
  validation.errors.push(message);
  validation.fieldErrors[field] = message;
}

function pushExtraEnvError(
  validation: GatewayProfileValidation,
  index: number,
  message: string,
) {
  validation.errors.push(message);
  validation.fieldErrors.extraEnv = "一个或多个附加环境变量需要修正。";
  validation.extraEnvErrors[index] = message;
}

function optionalTrimmed(value?: string | null): string {
  return value?.trim() ?? "";
}

export function validateGatewayProfile(profile: GatewayProfile): GatewayProfileValidation {
  const validation: GatewayProfileValidation = {
    ok: true,
    errors: [],
    warnings: [],
    fieldErrors: {},
    extraEnvErrors: {},
  };

  if (profile.name.trim().length === 0) {
    pushFieldError(validation, "name", "Profile 名称不能为空。");
  }

  if (!["splitter", "worker", "standalone"].includes(profile.runtimeRole)) {
    pushFieldError(validation, "runtimeRole", "Runtime role 必须是 splitter、worker 或 standalone。");
  }

  if (!Number.isInteger(profile.port) || profile.port < 1 || profile.port > 65535) {
    pushFieldError(validation, "port", "PORT 必须是 1 到 65535 之间的整数。");
  }

  const redisUrl = profile.gatewayRedisUrl.trim();
  if (redisUrl.length === 0) {
    pushFieldError(validation, "gatewayRedisUrl", "GATEWAY_REDIS_URL 不能为空。");
  } else if (!redisUrl.startsWith("redis://") && !redisUrl.startsWith("rediss://")) {
    pushFieldError(
      validation,
      "gatewayRedisUrl",
      "GATEWAY_REDIS_URL 应以 redis:// 或 rediss:// 开头。",
    );
  }

  const databaseUrl = optionalTrimmed(profile.gatewayDatabaseUrl);
  if (
    databaseUrl.length > 0 &&
    !databaseUrl.startsWith("postgres://") &&
    !databaseUrl.startsWith("postgresql://")
  ) {
    pushFieldError(
      validation,
      "gatewayDatabaseUrl",
      "GATEWAY_DATABASE_URL 应以 postgres:// 或 postgresql:// 开头。",
    );
  }

  if (optionalTrimmed(profile.gatewayManagementToken).length === 0) {
    validation.warnings.push(
      "未配置 GATEWAY_MANAGEMENT_TOKEN；启用管理鉴权时，停止操作会退化为强制终止。",
    );
  }

  const seenEnvKeys = new Set<string>();
  for (const [index, entry] of profile.extraEnv.entries()) {
    const key = entry.key.trim();
    const normalizedKey = key.toUpperCase();
    if (key.length === 0) {
      pushExtraEnvError(validation, index, `附加环境变量 #${index + 1} 的 KEY 不能为空。`);
      continue;
    }
    if (!ENV_KEY_PATTERN.test(key)) {
      pushExtraEnvError(
        validation,
        index,
        `附加环境变量 ${key} 不是有效 KEY；请使用字母、数字和下划线，且不能以数字开头。`,
      );
      continue;
    }
    if (RESERVED_ENV_KEYS.has(normalizedKey)) {
      pushExtraEnvError(validation, index, `附加环境变量 ${key} 是桌面启动器托管的保留 KEY。`);
      continue;
    }
    if (seenEnvKeys.has(normalizedKey)) {
      pushExtraEnvError(validation, index, `附加环境变量 ${key} 重复。`);
      continue;
    }
    seenEnvKeys.add(normalizedKey);
  }

  if (optionalTrimmed(profile.gatewayRoutesFile).length > 0) {
    validation.warnings.push("GATEWAY_ROUTES_FILE 会在保存时由桌面后端检查路径是否存在。");
  }

  if (optionalTrimmed(profile.workingDirectory).length > 0) {
    validation.warnings.push("Working directory 会在保存时由桌面后端检查路径是否存在。");
  }

  validation.ok = validation.errors.length === 0;
  return validation;
}
