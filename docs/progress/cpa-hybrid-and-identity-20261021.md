# CPA hybrid TLS 与身份模式续进

本记录接续 `cpa-rust-manual-20261021.md`，覆盖用户授权的两项实现。
这是开发检查点，不是完整兼容模式、移除 Go worker 或新手测包的交付声明。

## TLS 实现与本轮证据

- 固定 `rama-boring-sys 0.6.20` archive，SHA-256 为
  `ea17f614cc3eff7c149e8a68a4645fb6f556a31b768f6bbcb4162e6d42a2bba7`。
- 在上游现有 EC、ML-KEM 密码学原语之上增加两种 key-share 组合，未自行实现密码算法。
  `4587` 为 P-256 + MLKEM768，`4589` 为 P-384 + MLKEM1024；均使用 EC || ML-KEM。
- 组合类、独立 NID、factory、named groups、TLS 1.3 PQ 检查一起接线。
  `configure_context` 在没有兼容后端时失败，不再仅广告无法协商的组。
- `crates/cpa-bssl-sys` 只替换 rama sys；BSSL 环境覆写只存在于其独立 build-script
  进程内，不能给整个 Cargo 设置 `BORING_BSSL_SOURCE_PATH`，否则会污染 btls。
- 只读审查指出 receipt 漏掉实际执行的 `build/*.rs`。已改为校验整个准备 package，
  并增加离线回归：修改 build driver 后复用必须失败。1/1 通过。

实际运行结果：

| 场景 | 结果 |
| --- | --- |
| Go 服务端仅接受 4587，Go/Rust 客户端 | 均成功，均出现 HRR 和两次 ClientHello |
| Go 服务端仅接受 4589，Go/Rust 客户端 | 均成功，均出现 HRR 和两次 ClientHello |
| P-256、P-384 HRR 回归 | Go/Rust 各五次请求成功，解析后 ClientHello 一致 |
| Rust 真实 loopback，4587/4589 首包直接选组与 HRR | 全部成功，校验证书并交换应用数据 |
| TLS 1.2 服务端只配置两个 hybrid 组 | 握手均拒绝，不回退到不受支持的 PQ 协商 |
| 初始 ClientHello golden 与证书验证模式 | 通过 |

最终 `cargo test --manifest-path crates/cpa-tls-profile/Cargo.toml --locked` 为 3/3。
Go 互操作报告位于 `linshi/gateway-cpa-rama-http-20261021/extra-groups-report.json`
和 `hrr-report.json`。这证明实际协商，不等于密码协议全面审计。
畸形公钥、截断密文和恶意服务端输入的更细粒度负例仍需补齐。

构建需要 Python、Go object-table generator、git 和原生 BoringSSL 工具链；
这些是构建依赖，不是运行 Go transport worker。当前核验版本为 Python 3.12.10、
Go 1.26.1、Rust/Cargo 1.98.0。尚未承诺跨工具链二进制 bit-for-bit 可重现。
原生 prefix 当前来自 build-helper 的 0.1.0 版本，不应描述成上游原 prefix 不变。

## 可选 CPA Claude 身份模块

`crates/cpa-claude-profile` 已能独立编译和测试，尚未接入 Gateway 的 profile 选择、
发送路径和 UI，因此日常 `cpa` 行为没有被该模块更改。

已实现并测试的部分：

- credential 派生 device/account，显式 session 投影，以及普通 Messages root 派生；
- 多模态 root、未知 JSON block 排序和 cache_control 排除，避免图像会话每次随机变化；
- billing、CLI system 身份、日期提示和调用方 system 搬移的基础路径；
- 普通 Messages beta 顺序；显式缓存模式、四标记限制、保留优先级、TTL 顺序；
- 非法 metadata/body 拒绝且不留下半改写请求；空 seed / 未实现 OAuth 身份入口拒绝。

固定合成凭证的真实 CPA 抓包已抽取为
`crates/cpa-claude-profile/fixtures/ordinary-messages.json`，不含真实密钥。
固定日期、session 后，普通 `claude-opus-5` 的 body 结构、序列化字节、990 字节长度、
device/account 和 beta 顺序通过 golden 比较。此比较不证明 session 自动派生等价。
实际抓到并修复：缓存遍历通过可变 JSON 索引意外插入 `tools: null`，多出 13 字节。
也修复了 session label 在 trim 前未拒绝控制字符的问题。

最终身份模块测试 14/14 通过。这不等于完整模式端到端完成。

## 必须继续的接线与覆盖

1. 在认证后显式传递可信 caller scope。`stage_auth` 的 AuthenticatedSession 拥有
   project/tenant/access-key 身份；目前 request-plan 层拿不到它。不能把 raw_body、
   metadata、body.user 或未经证明的 x-neuro-user 当成可信作用域。
2. 增加显式 opt-in 配置和 UI，确保普通 CPA 默认行为不变，body/session/beta 在最终
   发送点一致，并完成流式/非流式及跨入站协议接线。不能因为风格开启 OAuth 生命周期。
3. 继续对照 native/helper/subagent、工具映射、动态 beta、advanced model、date-pin
   生命周期及 cache 边界；当前 `betas.rs` 仅覆盖普通路径，不能当完整通用算法。
4. 根 Gateway 编译、隔离真实 CPA/Gateway 多轮抓包和最终打包尚未执行。
   Cargo.lock 已最小更新，`cargo tree --offline --locked --features cpa-rust-transport
   -i rama-boring-sys --depth 1` 已确认指向本地兼容后端。
5. Go worker 仍保留；旧手测 EXE 不包含本轮改动。最终删除须满足技术验证和既定删除审批。

## 工程门禁

局部 rustfmt、Python integrity 测试、TLS 3/3、身份 14/14、checker 31/31、
effective-line ratchet、git diff --check 均通过。没有修改行数基线或历史例外。
没有运行全量 Gateway 测试、重新发布、部署、提交、推送或删除 Go worker。
