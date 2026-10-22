# CPA Rust 续进：头字段边界与手测候选

接续 `cpa-rust-runtime-20261021.md`。本轮不满足删除 Go worker 的条件，不能称为
完整网络指纹收敛；用户要求的最终删除/全等价交付仍未完成。

## 先失败再修复

- `src/upstream/request_profile/cpa_wire.rs`：排序时使用 HeaderMap::remove，只取到首值，
  导致重复头丢失。新增测试实际得到 1 而期望 2；改为按原值序列 append，保留敏感标志。
- `crates/cpa-http-transport/src/wire.rs`：Cookie 先前 trim_ascii 且保留末尾空段，
  不等于 Go net/http/internal/httpcommon 的分割规则。现在只去掉分号后的空格，
  保留内部空段，不发送最终空段；保留原敏感标记。
- 同文件：Go 只发送第一个 User-Agent；第一个值显式为空时不发送。之前 Rust
  保留所有值。新增两个 transport 测试均先失败，修复后 19 项 transport 测试通过。

实际 socket 对照：Go 和生产 Rust crate 探针，在 HTTP/2 与 HTTP/1.1 下分别执行
五次请求，均成功。重复头、Cookie 内部空段/尾空段、空 User-Agent 经源站接收后
一致。HTTP/1.1 普通头有序序列相同；HTTP/2 普通头按名称和值排序比较，不据此宣称
压缩字节、随机顺序与调度完全相同。四种代理矩阵重新运行，各五次仍成功。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-rama-http-20261021/`

- `header-edge-report.json`、`edge-go.json`、`edge-rust.json`
- `header-edge-h1-report.json`、`edge-h1-go.json`、`edge-h1-rust.json`
- `proxy-report.json`、`extra-groups-report.json`

## TLS 阻塞：不是配置开关

本轮重跑组 4587/4589：Go 成功并发送两次 ClientHello；Rust 在 HRR 后失败，
只发送一次。当前 rama-boring-sys 0.6.20 的 `SSLKeyShare::Create` 不支持这两个组。
raw supported groups 仅保留广告，不能产生缺失的密钥交换实现。

本轮还实际联网读取 BoringSSL 上游固定提交
`99b9d64ec02384c4c939f264b40e91d6cb5ca904` 的 `ssl/ssl_key_share.cc`，未找到
P-256+MLKEM768 / P-384+MLKEM1024 的对应组实现。未因此推断所有第三方分支都不支持。
没有自行拼装密码学实现、关闭验证、移除组后冒充相同，也没有隐藏 Go fallback。

## 完整 CPA CLI 模式尚未实现的依赖

本轮固定参考源码核验确认：完整模式不仅是一个 beta 列表，还包括 credential 派生
device/account、可信 session 来源、system/billing 改写、cache marker/TTL、按 body
选择 beta、continuity/date pin、部分目标的最终正文 CCH 签名及工具名映射。
真实 OAuth 的刷新和取消权限仍必须独立；不能因为启用风格就伪装 OAuth 生命周期。
因此没有发布一个仅增加静态头、却命名为“完整兼容”的开关。

## 手测交付范围

候选位置：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-rust-manual-20261021/`。
它是 headless Gateway 开发候选，启动器仅对子进程设置 Rust CPA transport、loopback
监听与独立 local 数据目录；不带真实凭证、不覆盖日常 .ng、不启动 Go worker。
没有完成全量测试/正式打包/部署；Go worker 源码及回退能力继续保留。
具体构建 SHA、运行验收与状态以候选目录 receipt 和后续本文件记录为准。

### 本轮最终验证

- 根 `cargo test --lib --features cpa-rust-transport cpa_`：3/3，通过重复头回归及
  原有 IPC 边界。transport 19/19、checker 31/31、ratchet、scoped rustfmt 和
  diff check 通过。没有声称 3576 项 Gateway 测试全部通过。
- `cargo build --bin gateway --features cpa-rust-transport` 成功，保留 24 个已有 lib
  warnings。新 EXE SHA-256：
  `b16c1b2b44a0931f1f0fc31573bd612fb730ff6c6eb36376c69c8b9de92ad1d6`。
- `gateway-e2e-20261021-194326`：真实 CPA 程序和新 Gateway EXE 各五次 200、
  正确 SSE、单连接。beta/session/content-length 及正文身份差异仍存在，原样报告。
- `gateway-proxy-e2e-20261021-194354`：新 Gateway EXE 使用隔离管理 API 导入
  合成凭证，经过 HTTPS+Basic、SOCKS5h+用户名密码代理，再通过校验证书的 TLS
  连接源站。各五次 200、正确 SSE、一个代理连接，无 Proxy-Authorization 泄漏。
  测试脚本首次因 Python capture 模块同名导入失败，修正测试 fixture 的 ROOT
  定位后通过；没有通过修改产品证书/认证逻辑绕过失败。
- 实际执行候选 `Start-Isolated-Rust.ps1`，独立 smoke data，不占用用户后续
  manual-data。healthz、/ui/、四个内嵌 JS 资源均 200；测试后精确子进程零残留。
  这里只验证 UI 资源可访问，不冒充浏览器交互人工验收。
- 手写相关有效行数：cpa_wire 89、wire 189、wire_edge_tests 44、lib 111；未扩张
  历史大文件、没有改行数基线、没有提交/推送/部署或移除 Go worker。
