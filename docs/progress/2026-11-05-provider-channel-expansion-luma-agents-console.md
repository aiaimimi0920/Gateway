# Luma Agents API：正常模板、Key 编辑与暂存受众保护

日期：2026-11-05。承接 [后端配置与保存子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-config.md)，对应 [总计划 R14/W10](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/plan/2026-11-02-provider-template-expansion.md)。
本批只闭合前端模板与 Key 编辑子链，**不代表完整 Luma 服务商、P50 或总目标完成**。工作区仍是本地 Gateway、`main`、HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`，保留既有 dirty edits。

## 改了哪些内容

- 正常目录开放独立 `lumalabs-api` 空池模板，固定 `https://agents.lumalabs.ai/v1`。旧 `lumalabs-web` / `lumalabs` 不变，不共享凭据、不自动回退、不预置模型或推定权益。
- 接入实际 `ConsoleGlobalDialogs`、普通 Key 表单和 controller。新增必须 Replace；编辑支持 Keep/Replace/Clear，无敏感权限可做 metadata Keep，新增或修改材料仍需原有权限。
- 普通管理 JSON 的 unset `null` 正常接受；provider 和最终 credential 的目标、Bearer、adapter/profile/family、全 Auto flow 与直接 HTTP 受约束，拒绝 OAuth/Web、refresh、任意 headers/extra_body、目录与其他执行路径。
- Key 使用后端同样的 `luma-api-*`、10–4096 ASCII 非空白字符规则；材料只进原 `CredentialSecretEdit` / `SecretPatch`，不写入公开配置或模板。
- 暂存绑定真实产品、固定受众和 Bearer，最终 patch 按稳定 provider/credential ID 重定位后复核。擦掉 preset、换地址/流程、删除身份或缺 binding 均不能带走暂存 Key；非 strict 预览省略不安全材料，真正保存明确报错。metadata Keep 保留此前暂存 Key，Clear 则按普通协议替换暂存操作。
- 仅手工支持当前实现的 `uni-1 / uni-1-max / ray-3.2`；不展示目录/拉取/自动发现控件，不进行网络发现或付费生成。映射 source 仍可为别名，例如 `my-image -> uni-1`，只限制上游 targets；没有修改全局 ModelIdPolicy 来误伤别名。
- 新产品按 preset 显示 Luma Agents 和媒体类别，用户复用旧 Web 或其他品牌 ID 也不能改变产品展示；不改变其他服务商的既有优先级，不增加“已验收”蓝标，不新增解释型 UI 或样式系统。

主要实现为 [lumaAgents.ts](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/lumaAgents.ts)、[模板接线](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/providerCatalog.ts)、[正常全局弹窗](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/ConsoleGlobalDialogs.tsx)、[编辑 owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/useCredentialDialogEditor.ts) 和 [最终 SecretPatch owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/apps/desktop/src/features/console/credentialDocument.ts)。

## 实际验证

证据目录：[gateway-luma-console-20261105](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-console-20261105)。

| 检查 | 结果与边界 |
| --- | --- |
| 写前真实红测 | 6 passed / 29 failed；模板、全局 UI、Key 绑定、非法模型/目标/流程均有真实失败，不是编译失败 |
| 展示身份补充红测 | 31 passed / 2 failed；旧 Web ID 覆盖新产品品牌、其他 ID 覆盖媒体类别，随后窄修并转绿 |
| 最终定向及相邻 UI | 71 文件，**382 passed / 0 failed / 1 skipped**，其中 **68 项新增测试**包含在总数内，不重复累计 |
| 跳过项 | 原有 `accepts an explicitly supplied private worker sample without printing it` 私有样本 opt-in；未启用、不计通过 |
| 官方 `npm run typecheck` | exit 0，最终源码复验 |
| 隔离 Rsbuild | 正常 `src/main.tsx` 与明确标识的 synthetic proof 双入口构建成功；没有加载 publisher，`cleanDistPath:false`，输出仅在 linshi |
| 浏览器 | 正常模板/GlobalDialogs/controller/SecretPatch builder；模型拒绝、暂存与 metadata Keep、最终 Replace/Keep/Clear、390px 窄屏、Escape 及焦点恢复均有截图/AX 证据。最终构建另确认模板→新增 Key→模拟 Replace，公开文档无原始材料 |
| checker / ratchet | checker 31 项通过；官方 ratchet exit 0，4516 文件、14 immutable runtime artifacts、governed source >700=0。共享报告原字节恢复 |
| 差异/编码/规模/来源 | 全工作区 `git diff --check` 与本批写前 scoped 差异检查通过；UTF-8 无 BOM、无尾随空白；最终相邻测试、类型检查和构建的 897 项选定输入均无运行中或最终漂移 |

浏览器 proof 使用真实生产组件，但保存 callback 是明确标注的合成行为，`client.request` 禁止管理/授权/目录调用；**没有实际连接管理 TCP、SQLite 或平台 API**。不能把它算生产管理端到端或真实生成验收。
首次浏览器使用旧元素索引遇到重绘后的 `Input is no longer connected`，刷新状态后继续，没有改生产代码来配合自动化。首次审计 helper 误把 `git diff --no-index --check` 的“有差异”exit 1当空白错误；确认输出为空后按该命令契约修正，仅修临时验证器。核对时也发现checker的JSON参数相对仓库根而非npm的cwd解析；最终回执已从实际 `artifacts/effective-code-lines.json` 收集，原有脏的 `scripts/artifacts/effective-code-lines.json` 原字节保留，不拿旧2529文件报告充当4516文件结果。
早期输入快照没有覆盖外部 `@neuro/ui`，最终三阶段已加入实际安装链接指向的 package/dist 原字节。此为897项选定输入证据，**不是 clean install 或整个 node_modules 冻结证明**；首次类型检查在补充测试时运行，不当最终证据。

本批 15 项实际新增/修改源码，另1项写前预留文件未修改。最大 `credentialDocument.ts` 为494→500有效行，模板481→484，新产品 helper98；未增加baseline/exception。该共享 SecretPatch 文件已到500线，后续再扩展应提取同职责校验，不能继续堆积。
沿用 `@neuro/ui`、既有 hooks 与表单；React复审覆盖派生状态、函数式更新、异步权限边界、标签/焦点/窄屏，不引入新库、Effect副本、网络等待或额外轮询。仓库未配置TypeScript formatter命令；未改Rust，本批不重复Cargo编译/测试或宣称新的Rust结果。

## 当前完成度与下一接力点

1. **本计划累计新增正常前端模板15→16**，这不是“16个原始需求全部完成”，也不新增R14/W10整卡offline_ready。
2. 下一步继续正常 main UI→真实管理 TCP/Local SQLite→保存/Keep/清空→同目录runtime重建；本批的合成保存不能替代它。
3. 精确媒体probe仍未完成。普通测试默认是文本 `Reply with only OK.`，不能自动变成付费图像/视频；需明确媒体操作/费用授权合同，再复用正式submit/poll/assets和精确账号/audit/取消owner。新增Unsupported gate不算probe完成。
4. 继续公共媒体router→普通Key/route→发送/响应→实际usage/audit/本地结算及失败/取消；供应商credits未知仍为null，0才是明确无扣款，不以底层执行链已有该字段就声称已入账。
5. Luma其他官方操作、HDR+10s文档矛盾、视频完整解码/播放、旧Web剩余，以及其他原定渠道/OAuth兼容恢复/Qwen受限项不变；P50全范围无凭据集成/devtest、P60集中录制、P70逐渠道真实模型/额度/费用验收继续。

本批没有读取或试用真实/历史凭据、调用付费平台、创建Redis/Docker资源、修改在线实例、出包、部署、提交或推送。两个临时loopback静态helper及本轮浏览器tab已关闭，viewport已复原；不复用已消费清理的P34容器授权。
