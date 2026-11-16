# 服务商分渠道扩展：第 23 批，Grok OAuth 独立协议底座

> 2026-11-03（Asia/Shanghai）；R05 / W03 / P20。审查问题已实际red并修补，最终v3定向验证74项通过，代表性产品feature关闭编译通过；不是 Grok OAuth 已可登录或整体 offline_ready。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`；不提交、推送、打包、重启或接管在线实例。

## 1. 转向未实现渠道，保留全部计划范围

P22 Bedrock 四类参数桥接已有92项实际离线通过，本批转向尚无专用实现的 Grok Build OAuth。它是 `https://auth.x.ai` 发行、`https://cli-chat-proxy.grok.com/v1` 业务受众的独立渠道，不把 Web SSO 或 xAI API Key 换标签使用，不改变既有池。

2026-11-03 再次无凭据联网读取官方 Grok Build pinned commit `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8` 的 LICENSE/config/OIDC protocol/device_code/auth_credentials 与公开 OIDC discovery，全部 HTTP200。文件、精确来源 URL 和 SHA-256 存在 [P23 证据目录](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261103-p23) 的 `reference/` 和 `references.json`。Apache-2.0 源码只作协议行为参考，没有复制源码、安装依赖或执行外部项目。

官方 discovery 确认 authorize/token/device endpoint、PKCE S256、三种 grant，以及 ES256 的 ID-token 算法。公开第一方 client ID 不自动表示 Gateway 获准使用该身份。本底座要求显式提供 client/scopes，不内置官方 CLI client、不伪装 `x-grok-client-version` 或 `X-XAI-Token-Auth`。厂商客户端适用边界在真实登录前仍需核实。

## 2. 当前实现边界

新增 `src/protocol/grok_oauth.rs` 与独立 authorization/material/transport owners：

- 生产 issuer 固定，OAuth URL/POST path 不由导入材料覆盖；假 issuer 只在单元测试中开放并限随机127.0.0.1。
- PKCE verifier/state/OIDC nonce 分开随机生成，loopback `/callback` 显式端口；callback 限长、重复字段/错目标/错state/错误与code冲突拒绝。
- 授权码 exchange、refresh、device issue/token poll 使用各自准确 form；refresh 保留显式 principal_type/principal_id。
- 禁用共享默认请求头、禁止重定向、总deadline15秒、decoded累计64KiB、重复顶层JSON字段拒绝；没有自动重试code/refresh，也没有外泄上游错误prose。
- 设备授权提供单个有间隔的poll step，pending/slow_down（+5秒）/拒绝/过期分开；本地最多600秒且不延长server更短TTL；没有另起无限task，future drop释放等待或连接。
- token类型明确命名 `UnverifiedTokens`，不将transport成功当作已认证身份；token、code、device材料的Debug均脱敏。

**尚未接入：**JWKS/ES256签名与issuer/audience/nonce验证、实际consent principal确认、Console session/管理授权/单次callback、设备会话及退出join、普通秘密提交、独立preset/模板/Responses调用、共享refresh scheduler与generation/CAS持久化。本底座没有开放一个不完整的正常登录入口。

## 3. red、审查与验证状态

`red-grants-v1.log` 实际 exit101，尚无生产owner使新测试不能编译（E0425/E0433）；同时暴露一个测试fixture名称遮蔽问题（E0618），已修正。它是缺失实现/编译红测，不伪装已复现旧生产运行缺陷。

新单元合同覆盖RFC7636向量、独立state/nonce、精确grant form/转义、默认认证头不继承、principal/client受众、错误固定脱敏、坏响应/重复JSON/UTF8/截断/超限、设备间隔/终态、取消连接释放。所有HTTP测试仅随机loopback与合成client/token，不访问真实授权或业务上游。

初版 `green-grants-v1.log` 已实际完成：编译13m54s，**16 pass / 0 fail / 0 ignore**，exit0。`run-static.ps1 -Version v1` 实际exit0：官方Rustfmt、checker31/31、无报告ratchet、tracked diff通过。这不是最终审查修补后的来源。

独立只读审查发现初版DeviceChallenge只绑定client_id/issuer，缺principal/scope快照。`red-device-binding-v1.log` 已实际完成：编译11m17s，**0 pass / 1 fail / 0 ignore**，exit101；本地fixture实际收到了设备grant，并返回 `Ok(Authorized(UnverifiedGrokTokens { material: <redacted> }))`，预期为 `Err(InvalidInput)`。循环在principal分支首个断言失败，不冒称scope分支也已实际执行red。

修补后Authorization和DeviceChallenge都保存完整不可变OAuthClient快照；相等比较覆盖client_id/issuer/scope/principal，在等待及网络前拒绝重绑定。设备响应后再验TTL；device wire仍沿官方公开的client_id/scope/referrer，不臆造principal参数。consent实际principal的确认仍归后续身份owner，不把预选择当强制pin或已验签身份。独立复审确认接线修复，未跑测试，不将源码复审算动态通过。

最终测试按职责分为授权/设备状态、结构化HTTP wire、原始HTTP framing/资源生命周期；raw fixture先有界读完完整请求，避免将残留request body误判成取消后连接未关。设备expiry/取消负测即便意外发送也只到loopback，不使用真实issuer兜底。新增15秒自身deadline覆盖stalled body的合同。最终 `green-grants-v3.log` 已实际完成：编译10m23s，运行15.03s，**18 pass / 0 fail / 0 ignore**，exit0。

受影响回归 `refresh-adjacent-v3.log` 为16 pass，`call-flow-adjacent-v3.log` 为40 pass；三个过滤器互不重叠，`test-results-v3.json` 合计 **74 pass / 0 fail / 0 ignore**，不累计初版green或red重跑。`feature-disabled-v3.log` 对应 `cargo check --locked --jobs 4 --no-default-features --features line-gemini-web-reverse --lib` 已exit0，用时1m57s；仅保留Gemini，证明新底座不依赖Grok Web等产品feature，不代表此前core-only Gemini缺失符号或全部feature矩阵已解决。

`run-static.ps1 -Version v3` 已exit0：官方Rustfmt、checker **31/31**、无报告ratchet **4039文件 / 14 immutable / governed source >700=0**、tracked whitespace通过；最终源码hash与本批untracked whitespace由 `run-source-final.ps1` 复核通过。现有warning未越界修改，没有新增依赖或改动有效行baseline/豁免。

`source-before.json` 对此前干净的protocol/mod.rs采用HEAD原文（90有效行），所有本批新增owner前值为0；这是结合Git状态的可重建前值，非声称改动前已保存逐文件快照。v1为首次green的8文件来源，v2加入审查负向合同；v3冻结最终9文件，新增raw_transport测试owner前值为0。v1/v2和失败日志保留，不复用旧hash充当最终来源。

v3有效行：已有protocol/mod.rs 90→91；新root114、authorization126、material170、transport190、tests171、wire184、adversarial125、raw_transport142。无文件超过250有效行，无例外或baseline变更。

## 4. 不变的剩余项

API Key/OAuth/Web继续按实际产品与额度区分，未知关系不合池，Web不是兜底。所有R01–R16与额外同平台分支仍保留；Qwen原OAuth停用约束也未移除。仍未读取/试用历史凭据，没有真实授权/刷新/生成、费用、EXE包、部署或在线数据库变更。

W03完整生命周期及其他OAuth/API/Web/媒体、W05/W06剩余保真和PG边界、既有feature/前端/expiry未通过项、P50打包/隔离启动、集中录制与逐渠道真实验收均开放，总goal保持active。
