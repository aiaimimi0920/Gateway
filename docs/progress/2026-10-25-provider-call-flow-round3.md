# 三适配器框架第三轮复核

## 范围与修复

本轮重新检查请求级准备、快捷发送和新旧策略字段的合并边界，修复两处可复现问题。
保留既有未提交的界面、凭据池、4300 手测隔离和前两轮框架修改；不提交、不推送、不出包。

1. **Hosted 凭据指纹覆盖失效**：服务商 flow 指纹为 Claude、凭据旧 profile 指定 Codex/Auto 时，
   JSON 深合并只替换旧字段，后续 prepare 又按继承 flow 覆盖它，与 YAML configure 合同不同。
   修复在共享 merge 仍保有来源时同步凭据指纹，两条 Hosted 路由读取路径共同受保护。
   凭据自有的新 flow 指纹仍优先于同层旧镜像；无凭据覆盖时仍以服务商 flow 为准。
2. **跨层 alias 形成重复字段**：服务商与凭据分别使用 snake_case/camelCase 的合法策略字段，
   深合并后可能同时留下两个名字，typed reader 报 `duplicate field`，无法生成候选。
   现在仅分别规范化两层已支持的两个顶层 alias，再执行原深合并。

生产修改集中在 `src/db/provider_credentials/payload.rs`。不改变密钥、鉴权目标、模型规则、
额度来源或凭据身份，不重写数据库或配置，不更改持久化版本，也不新增秘密日志或网络操作。
规范化使用 Cow：普通 canonical 凭据保持借用，只有需要处理 alias 才复制该层 JSON。

## 拒绝与兼容边界

- 同一层同时出现 canonical 与 alias 仍视为重复字段，不按遍历顺序择一。
- 同步前校验 merged flow，防止旧 profile 把 inherited unknown/null/非法类型修成合法策略。
  未知字段原样保留并拒绝，不通过重建已知字段来丢弃错误。
- 凭据自己的 fingerprint 按 key 存在判断；显式 null/unknown 不被旧字段替换。
- profile 为 null/空白时不合成指纹；无 flow/null flow 不合成新 flow，沿用既有语义。
- 专用执行器仍拒绝不支持的 API 指纹，不通过忽略凭据设置来假装成功。
- Redis 扁平 payload 已失去来源，不在 hydration 盲目提升旧 profile 优先级。
  已有 flow 权威性、discovery 信任边界和模型负证据保护不变。

## 实际验证

先增加共享 merge 回归并增强真实 Hosted builder 断言；修复前 33 项中 3 项失败，分别复现
Auto 被覆盖、duplicate field、Hosted Codex 被覆盖。修复后再增加专用会话拒绝保护，最终
`cargo test --locked --lib call_flow --jobs 4 --no-default-features --features runtime-defaults`
为 **34 项通过，0 失败**。新增共享 merge 测试共 7 项，覆盖跨层 alias、凭据优先级、同层新旧
字段冲突、无 flow、null、未知策略、会话执行器，使用合成数据，不连接数据库。

| 定向检查 | 本轮结果 |
| --- | --- |
| `db::provider_credentials` | 12 项通过 |
| `db::routing` | 19 项通过，包含真实 Hosted builder 与所选密钥断言 |
| `routing::config` | 96 项通过 |
| `routing::credential_routing` | 29 项通过 |
| `routing::protocol_resolution` | 34 项通过 |
| `provider_credential_refresh` | 3 项通过 |
| `normalize_chatgpt_codex` | 3 项通过 |
| `provider_discovery` | 9 项通过 |
| `upstream::chatgpt::official_api` | 36 项通过 |
| `pipeline::stage_send::tests` | 39 项通过 |
| 全目标编译 | `cargo check --locked --all-targets --jobs 4 --no-default-features --features runtime-defaults` 通过 |
| 规范 | Rustfmt check、checker 自测 31 项、有效行 ratchet、UTF-8 无 BOM、`git diff --check` 通过 |

过滤器有交集，不相加作为独立覆盖总数。call_flow 套件同时重跑前两轮的随机回环发送、
official CPA 准入和 FreeBuff 直达拒绝保护；不把 CPA 准入测试说成 CPA Rust 成功发送验证。
全目标编译保留原有 unused/dead-code warning，不清理无关 Gemini executor。

本轮生产文件 115 → 165 有效行，已有 Hosted 测试 128 → 151，新测试文件 164，均不超过 250。
已检查相对修改前快照的生产 diff，并完成独立只读交叉复核；没有修改基线、豁免或依赖锁。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-call-flow-round3/`。
核心记录为 `red-rust.log`、`green-rust.log`、`affected-results.json`、`check-all-targets.log`、
`production-round3.diff`、`before-counts.json`、`after-counts.json`、`source-validation.json`，
以及 formatter、checker、ratchet、diff 和依赖锁检查日志。

## 未执行与剩余验收

- 未连接 4200/4300，未读取 `.ng` 或真实凭据，未访问付费上游，未修改运行实例。
- 未启动 PostgreSQL/Redis，Hosted 证据为真实生产函数上的合成 row 测试，不冒充真实服务链验收。
- 本轮没有修改前端生产源码，未重跑前端测试，不将上一轮结果记为本轮新结果。
- 未运行全量业务回归、逐模板账号/浏览器会话验收、正式发布审计或产品 EXE 构建打包。
- `TEMPLATE_ADAPTATION_RECEIPTS` 仍为空，框架复核不等于逐模板完成，不新增蓝色验收标记。
