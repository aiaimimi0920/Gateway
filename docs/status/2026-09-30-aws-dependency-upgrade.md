# AWS S3 依赖升级检查点，2026-09-30

本批接续 [spin 兼容补丁检查点](2026-09-30-dependency-spin-closure.md)，
只处理 AWS SDK 引入的受影响 LRU 依赖。保留 S3、S3-compatible 和原有
SigV4a 能力，不重写存储接口，不修改生产 Rust 逻辑或 S06 Rust/Gemini owner。
安全审计仍失败，不将本批标记为整个依赖安全方案或正式发布已完成。

## 变更与解析证据

- 根 `aws-sdk-s3` 从 `1.119.0` 升至 `1.144.0`，manifest 升级下限为 `1.144`；
  `default-features = false` 和 `sigv4a`、`default-https-client`、`rt-tokio` 原样保留。
- AWS 引入的 `lru 0.12.5` 已从根锁文件移除，解析为 `lru 0.18.5`。
  两项独立公告的修复下限分别为 `0.16.3` 和 `0.18.2`；新合同采用较严格下限。
- SDK 的声明 MSRV 为 `1.94.1`；根 manifest、仓库工具链、CI、Windows 发布入口、
  Docker builder 和开发容器统一固定为已安装的 Rust `1.95.0`。README 要求同步更新。
  独立 `gateway-local-data` 库不依赖 AWS，保留自身 `1.91.1` MSRV。
- 锁文件通过官方 Cargo 更新，不手改版本或 checksum。AWS 同族和必要 crypto
  依赖一起更新，新增 package/version 37 项、移除 35 项；`rquest` package 原样保留。
- 缓存 SDK archive 的 SHA-256 与 Cargo lock 一致：
  `30dc8bf6baaf7d46336a0ca2c69f223d9b90d7a801fb3e28f7ea17b00dc6b1de`。
  精确版本的 crates.io API 联网核验遇到 TLS 握手失败；不称本批完成新鲜元数据核验，
  也不称所选版本是已经联网证明的最新版本。
- Docker Hub 的 `rust:1.95.0-bookworm` tag 已联网核实，保存 digest：
  `sha256:6258907abe69656e41cd992e0b705cdcfabcbbe3db374f92ed2d47121282d4a1`。
  这只证明 tag 存在，不证明 Docker 镜像已经构建或运行。

## 聚焦验证

本批使用 `CARGO_BUILD_JOBS=1`、`CARGO_INCREMENTAL=0`、`--offline --locked`，
所有新测试数据、临时文件和 Cargo target 都放在 `linshi`；没有使用真实云端凭据。

| 检查 | 本批结果 |
| --- | --- |
| Windows 根 `cargo check --all-targets` | 通过 |
| 对象存储库测试 | 28/28，通过；包括新增 HEAD readiness、403 拒绝和 deadline 回归 |
| Bedrock 协议与 SigV4 相关测试 | 14/14，通过 |
| SQLite 本地流水线合同 | 3/3，通过 |
| Windows Tauri `cargo check --offline --locked` | 通过；桌面 manifest 和 lock 未改动 |
| AWS dependency contract | 4/4，通过；包含工具链、入口一致性、S3 features 和 LRU floor |
| Docker / standalone contracts | 23/23、15/15，通过 |
| Security contracts / checker tests | 15/15、31/31，通过 |
| 行数 ratchet / strict | 通过；2604 文件、14 个单列 immutable runtime assets，受治理源码 `>700=0` |
| 根与 Tauri formatter、五个 workflow 的 actionlint 1.7.12 | 通过 |
| `git diff --check` | 通过；最终文档加入后再次核对 |

运行中遇到的失败没有抹除：

1. 离线缓存缺少 9 个必要 crypto 包，受限联网 `cargo fetch --locked` 成功补齐；
   只在该进程清空 proxy 环境，没有修改全局设置或关闭证书验证。
2. BoringSSL CMake/MSBuild 的 FileTracker 报 `FTK1011`。失败 tracker 路径为
   262 字符；改用 `linshi\g95-073651` 的短 target 后为 210 字符，编译通过。
   没有删除旧 cache、修改注册表或创建网络盘。
3. 新工具链编译集成测试时出现 `queries overflow the depth limit!`。
   仅在 `provider_credential_folder_sync_runtime` 测试 crate 补齐与生产库相同的
   `recursion_limit = "256"`，重新 all-targets 检查通过；没有修改测试运行逻辑。
4. 新增 readiness fixture 初次按无尾斜杠 HEAD bucket 路径匹配，25 项原回归通过、
   3 项新测试失败。实际请求路径为 `/fixture-bucket/`；修正 fixture 匹配和精确断言后
   28/28 通过。没有修改生产路径构造或弱化失败断言。

## S3-compatible 兼容性边界

缓存源码复核确认，旧新 smithy `BehaviorVersion::latest()` 都为 `v2026_01_12()`；
PUT 默认 CRC32、内存 body 的 header checksum 和签名选项未变。
Gateway 的 endpoint、region、force-path-style、credentials 和 timeout owner 未改动。

新 GET 支持额外验证 `sha512`、`md5`、`xxhash64`、`xxhash3`、`xxhash128` 响应 checksum。
若第三方 endpoint 返回不一致的相应 header，新客户端可能拒绝旧客户端未验证的响应。
本批没有真实 R2 header 样本，不据此推断实际故障，也没有关闭校验来掩盖兼容风险。
生成 endpoint resolver 有较大变化，未逐条证明规则等价。

loopback fixture 证明 path-style 方法、路径、收发数据和有界失败行为，但不验完整
SigV4 签名；默认非 path-style 的真实 R2 DNS/HTTPS、云端权限和读写仍未验收。
Bedrock 的本地签名回归也不是一次真实 AWS 调用。

## 审计仍失败

根锁文件使用保存的 RustSec 数据库复扫，`cargo audit` 退出码为 **1**，ignore 列表为空：

- vulnerability：`rsa 0.9.10 / RUSTSEC-2023-0071`，仍 1 项；
- unsound：从 4 项降为 2 项，剩余都是 `lru 0.13.0 <- rquest 5.1.0`，对应
  `RUSTSEC-2026-0002` 和 `RUSTSEC-2026-0253`；
- yanked：`rquest 5.1.0`，仍 1 项。

数据库 revision 的保存记录为 `9b3a3b73a7f42606494c943e95f8196e9994df46`；
本批没有成功联网更新数据库。这不是一次新鲜 GitHub OSV、Gitleaks 或 CodeQL run。
没有增加 ignore、虚构审批或放松发布安全门禁。

`rquest` 的 LRU 是非 optional 且在连接池中实际使用；关闭 feature 或运行时禁池
不能清除依赖图。已联网取得的候选矩阵显示：`wreq 0.15.3` 仍依赖 LRU 0.13，
稳定 `0.16.1` 要求 Rust 1.98，`6.0.0-rc.31` 是底层 protocol/runtime 已变化的 RC。
本批不据此实施未经 API、TLS 指纹和流式兼容证明的客户端替换。

## 行数、资源与保留边界

| 手写测试 owner | 修改前有效行 | 修改后有效行 |
| --- | ---: | ---: |
| `src/object_storage/tests/s3.rs` | 207 | 254 |
| `tests/provider_credential_folder_sync_runtime.rs` | 102 | 103 |
| 新 AWS dependency contract | 0 | 44 |
| Docker dependency contract | 391 | 391 |
| standalone repository contract | 484 | 484 |

全部不超过 500 行；S3 fixture 仍围绕一个存储 HTTP 回归职责，没有新增软例外。
fixture 只绑定 loopback，使用合成凭据、有限请求和原有有界 body；Drop abort server，
超时通过现有取消 owner 处理。没有新生产任务、队列、secret 日志或阻塞热路径。
配置修改不增加 token 权限、不关闭 TLS、checksum 或审计。修改文本已检查 UTF-8 无 BOM。

两个 detached Cargo lock、桌面 manifest 和原有三个 dirty cache 的 hash 均保持；
Git source diff 中只有 S3 测试发生变化，生产 Rust 和 S06 scope 未改动。
Neuro 根仓库与其他子仓库的既有 dirty 状态保留，没有跨仓修改。

## 回滚与停止条件

本批是源码阶段，不涉及数据库 schema、凭据迁移或在线实例切换。回滚时只按本批
owned 文件清单协调恢复工具链、manifest、lock 和入口 pin，使用证据目录的 `.before`
文件保留此前未提交的安全门禁修改；不要对工作树执行 blanket reset。
旧依赖有已知公告，恢复旧组合不等于安全通过，更不能解除发布阻塞。

本批不提交、不推送、不重启 Docker、不修改 `.ng` 或真实数据库，也不生成
安全审计仍失败的新正式 release。Linux/Docker 构建、真实 R2、原生 UI、S06/S18
集成与完整产品验收没有从上述局部门禁推导通过。下一独立批再处理剩余 owning-parent
迁移或依赖移除，不自动升级至 Rust 1.98 或采用 RC 客户端。

精确命令、初次失败、最终回执、源码 checksum 与兼容性复核保存在：

`C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-dependency-migration-20260930-073651`
