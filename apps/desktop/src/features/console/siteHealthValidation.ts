import type { SiteHealthConfig } from "./siteHealth";
import { officialHealthAdapter } from "./officialHealthAdapters";

// Mirror structural constraints for immediate feedback; the backend remains authoritative.
export function validateSiteHealthPlans(
  config: SiteHealthConfig,
  groupIds: ReadonlySet<string>,
) {
  const ids = new Set<string>();
  const assigned = new Set<string>();
  for (const profile of config.profiles) {
    if (ids.has(profile.id)) throw new Error("检测方案 ID 不能重复。");
    ids.add(profile.id);
    if (!profile.account_group_ids.length && config.profiles.length > 1)
      throw new Error("全部分组共享方案不能与其他检测方案并存。");
    for (const group of profile.account_group_ids) {
      if (!groupIds.has(group)) throw new Error(`本地凭证组 ${group} 不存在。`);
      if (assigned.has(group))
        throw new Error(`本地凭证组 ${group} 不能重复绑定检测方案。`);
      assigned.add(group);
    }
  }
  for (const policy of [config, ...config.profiles.map((p) => p.policy)]) {
    const official = officialHealthAdapter(policy.adapter);
    if (official && ((policy.site_url != null && policy.site_url !== official.url) || policy.auth.kind !== "none"
      || policy.allow_private_network || policy.monitor_groups.length || Object.keys(policy.model_map).length))
      throw new Error("官方状态适配器使用固定公开地址，无需认证，不支持自定义监控分组或模型映射。");
    if (policy.adapter === "auto" && policy.auth.kind !== "none")
      throw new Error(
        "使用监控认证前必须手动选择适配器，自动识别不会广播密钥。",
      );
    if (
      ["sub2api", "sub2api-v2"].includes(policy.adapter) &&
      policy.monitor_groups.some((g) => !/^[1-9]\d*$/.test(g))
    )
      throw new Error("sub2api 监控分组必须填写正整数 group ID。");
    if (
      policy.auth.kind === "api_key" &&
      !["x-api-key", "api-key"].includes(
        policy.auth.header?.toLowerCase() ?? "",
      )
    )
      throw new Error("监控 API key header 只能是 x-api-key 或 api-key。");
  }
}
