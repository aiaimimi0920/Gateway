import { useEffect, useState } from "react";
import { officialHealthAdapter } from "./officialHealthAdapters";
import {
  probePolicySchema,
  siteHealthConfigSchema,
  type ProbePolicy,
  type SiteHealthConfig,
} from "./siteHealth";
import {
  healthSecretAudience,
  validateHealthSecretChange,
  type SiteHealthSecretChange,
} from "./siteHealthSecretDraft";

type TextFields = { groups: string; mapping: string };
type SecretInput = {
  operation: "keep" | "replace" | "clear";
  value: string;
  audience: string;
};
const key = (id: string | null) => JSON.stringify(id);
const textFor = (policy: ProbePolicy): TextFields => ({
  groups: policy.monitor_groups.join(", "),
  mapping: Object.entries(policy.model_map)
    .map(([a, b]) => `${a}=${b}`)
    .join("\n"),
});
const cleanConfig = (provider: Record<string, unknown> | undefined) => {
  const result = siteHealthConfigSchema.safeParse(provider?.site_health);
  return result.success ? result.data : siteHealthConfigSchema.parse({});
};

/** One unsaved form per provider; explicit plans keep independent text and secret drafts. */
export function useSiteHealthForm(
  provider: Record<string, unknown> | undefined,
  hasSecretAccess: boolean,
) {
  const [config, setConfig] = useState(() => cleanConfig(provider));
  const [enabled, setEnabled] = useState(() => provider?.site_health != null);
  const [selectedId, select] = useState<string | null>(
    () => config.profiles[0]?.id ?? null,
  );
  const [texts, setTexts] = useState<Record<string, TextFields>>({});
  const [secrets, setSecrets] = useState<Record<string, SecretInput>>({});
  const [dirty, setDirty] = useState(false);
  useEffect(() => {
    const next = cleanConfig(provider);
    setConfig(next);
    setEnabled(provider?.site_health != null);
    select((current) =>
      next.profiles.some((p) => p.id === current)
        ? current
        : (next.profiles[0]?.id ?? null),
    );
    setTexts({});
    setSecrets({});
    setDirty(false);
  }, [provider]);
  useEffect(() => {
    if (!hasSecretAccess) setSecrets({});
  }, [hasSecretAccess]);
  const profile = config.profiles.find((p) => p.id === selectedId);
  const policy = profile?.policy ?? probePolicySchema.parse(config);
  const text = texts[key(selectedId)] ?? textFor(policy);
  const secret = secrets[key(selectedId)] ?? {
    operation: "keep" as const,
    value: "",
    audience: "",
  };
  const changeConfig = (next: SiteHealthConfig) => {
    setEnabled(true);
    setConfig(next);
    setDirty(true);
  };
  const changePolicy = (next: ProbePolicy) => {
    if (
      next.adapter !== policy.adapter &&
      officialHealthAdapter(next.adapter)
    ) {
      setTexts({ ...texts, [key(selectedId)]: { groups: "", mapping: "" } });
      // Discard unsaved plaintext, but never clear a persisted secret without authorization.
      setSecrets({
        ...secrets,
        [key(selectedId)]: { operation: "keep", value: "", audience: "" },
      });
    }
    changeConfig(
      profile
        ? {
            ...config,
            profiles: config.profiles.map((p) =>
              p.id === selectedId ? { ...p, policy: next } : p,
            ),
          }
        : { ...next, profiles: config.profiles },
    );
  };
  const changeText = (next: TextFields) => {
    setTexts({ ...texts, [key(selectedId)]: next });
    setDirty(true);
  };
  const changeSecret = (next: Pick<SecretInput, "operation" | "value">) => {
    if (!hasSecretAccess) return;
    setSecrets({
      ...secrets,
      [key(selectedId)]: {
        ...next,
        audience: healthSecretAudience(provider ?? {}, policy),
      },
    });
    setDirty(true);
  };
  const build = (): {
    config: SiteHealthConfig | null;
    changes: SiteHealthSecretChange[];
  } => {
    if (!enabled) return { config: null, changes: [] };
    const changes: SiteHealthSecretChange[] = [];
    const compile = (policy: ProbePolicy, id: string | null) => {
      const fields = texts[key(id)] ?? textFor(policy);
      const model_map = Object.fromEntries(
        fields.mapping
          .split("\n")
          .filter((line) => line.trim())
          .map((line) => {
            const at = line.indexOf("=");
            const a = line.slice(0, at).trim();
            const b = line.slice(at + 1).trim();
            if (at < 1 || !a || !b)
              throw new Error("模型映射格式：本地模型=站点模型");
            return [a, b];
          }),
      );
      const secret = secrets[key(id)];
      if (secret?.operation === "keep")
        changes.push({ profileId: id, operation: "keep" });
      else if (secret) {
        if (!hasSecretAccess)
          throw new Error("编辑监控密钥前需要先确认敏感信息访问权限。");
        if (secret.audience !== healthSecretAudience(provider ?? {}, policy))
          throw new Error("监控目标或认证已改变，请重新替换或清空监控密钥。");
        const change: SiteHealthSecretChange =
          secret.operation === "replace"
            ? { profileId: id, operation: "replace", value: secret.value }
            : { profileId: id, operation: "clear" };
        validateHealthSecretChange(change);
        changes.push(change);
      }
      return {
        ...policy,
        model_map,
        monitor_groups: fields.groups
          .split(",")
          .map((s) => s.trim())
          .filter(Boolean),
      };
    };
    const next = {
      ...compile(probePolicySchema.parse(config), null),
      profiles: config.profiles.map((p) => ({
        ...p,
        policy: compile(p.policy, p.id),
      })),
    };
    return { config: siteHealthConfigSchema.parse(next), changes };
  };
  return {
    config,
    enabled: enabled && policy.enabled,
    selectedId,
    profile,
    policy,
    text,
    secret,
    dirty,
    select,
    build,
    changeConfig,
    changePolicy,
    changeText,
    changeSecret,
    setEnabled: (value: boolean) => {
      // Pausing persists configuration and secret identity instead of deleting site_health.
      setEnabled(true);
      changePolicy({ ...policy, enabled: value });
    },
  };
}
