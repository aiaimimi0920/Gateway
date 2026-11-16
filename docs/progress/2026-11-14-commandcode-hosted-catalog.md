# Command Code Hosted 目录接线

## 已复现的问题与修复

DB row／credential reader 会丢弃 raw `model_catalog`，通用 OpenAI surface 又会过滤 Messages。
本轮四项红测全部失败，分别覆盖无效目录、声明能力、显式协议策略和旧账号能力范围。
日志：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113/commandcode-hosted-red.log`。

修复采用单一 `catalog::hydrate`：DB reader、兼容 payload reader 和路由准备入口在产品验证前
恢复目录；校验固定来源、目录模型与端点一致性。raw `extra_body` 非空时拒绝，不先覆盖后放行。
内部 metadata 不进入推理正文。重复投影同一份 raw 输入保持一致。

协议策略仍由原来的 model-policy owner 处理，新增入口仅允许传入产品声明的能力范围。
先使用目录能力作为 surface，再应用原有 model/global allowed、blocked 与 wildcard 规则；
空交集直接拒绝，不回退全部能力。无目录／目录外手工模型保留 Chat，不猜原生能力。

凭证提供新 `model_catalog` 时整体替换账号层旧目录，不递归拼接旧模型的 endpoint map。
这一共享行为按目录快照语义处理，不改凭据 Key 或其他普通字段的既有 merge 规则。

交叉审查发现无 preset 的缓存 raw 账号还会丢失 `auth_mode`，因此同一个 hydrate 也恢复明确的
Command Code 身份，并拒绝冲突身份和重复别名。新增测试实际调用 `credential_to_candidate`，
覆盖有／无 preset、snake/camel 身份、精确缓存 Key 和 blocked family；不是仅构造 typed payload。

## 验证与边界

最终产品回归 `commandcode-hosted-green.log`：**57 passed / 0 failed**，其中新增 Hosted 接线测试
8 项全部通过，包含实际缓存转换入口。测试执行 71.01 秒；编译 19m15s 包含等待共享 target 锁，
不是纯编译性能结果。最后源码修改早于该 rustc 进程启动，新增 8 项均存在于最终回执。

复用同一最终测试 EXE 的必要共享回归：

| 范围 | 结果 | 日志 |
| --- | --- | --- |
| 协议选择／model policy | 36 passed | `commandcode-hosted-shared-0.log` |
| Hosted call flow | 3 passed | `commandcode-hosted-shared-1.log` |
| 凭证 merge／身份上下文 | 20 passed | `commandcode-hosted-shared-2.log` |
| payload candidate | 17 passed | `commandcode-hosted-shared-4.log` |
| 缓存凭证转换 | 29 passed | `commandcode-hosted-cache.log` |
| SenseNova 定向回归 | 24 passed | `commandcode-hosted-sensenova_.log` |
| SAP AI Core 定向回归 | 26 passed | `commandcode-hosted-sap_aicore_.log` |

`credential_to_candidate` 名称筛选曾匹配 0 项，不作为通过证据；核对实际测试列表后改用完整模块，
得到上表 29 项结果。
后两项仅确认共享接线未破坏现有产品测试，不代表其整项功能已完成。

独立 feature 编译 `commandcode-hosted-feature.log` 通过（exit 0）：
`cargo check --locked --offline --lib --bins --no-default-features --features line-commandcode-provider-api -j 2`。
最终 checker／ratchet 通过，见 `commandcode-hosted-checker-final.log`、
`commandcode-hosted-ratchet-final.log`；未调整 baseline／例外。定向 rustfmt、UTF-8 无 BOM、
`git diff --check` 与源码／测试 EXE SHA-256 分别见 `commandcode-hosted-source-checks.txt`、
`commandcode-hosted-sha256.txt`。本轮有效行最高为现有 `candidate.rs` 的 447 行；没有文件跨越 500 行。

本轮测试调用真实 DB row／credential builder、兼容 reader、缓存转换入口、最终协议选择与
请求 plan，但没有连接 PostgreSQL／Redis 服务，也没有把这组测试称为 Server 部署验收。
此前 Local 合成上游与重启证据继续保留；Server 实例、多实例及打包 EXE 验收仍开放。

### 2026-11-14 后续环境核查

本地已安装 Docker CLI，但 `docker version --format '{{.Server.Version}}'` 返回 exit 1：
`open //./pipe/dockerDesktopLinuxEngine: The system cannot find the file specified.`
因此本检查点无法直接运行隔离 PostgreSQL／Redis 验收，未启动或重启 Docker，也未触碰既有容器。
`Get-Command docker,redis-server,psql` 仅发现 Docker；这只表示另外两个命令当前不在 PATH，
不推断机器上完全没有安装。

待补证据仍是实际 Hosted reader/cache 到合成 Command Code 上游的发送、审计和用量链路，
以及第二实例读取同一配置和调度槽。已有 `SET ... NX EX` 实现不代替双实例实测；
不新增“整轮账号永不并发”的保证，遵循 `docs/credential-test-policies.md` 已有调度槽边界。

不变更真实账号、
凭据、数据库、运行实例，不执行真实付费上游调用。
