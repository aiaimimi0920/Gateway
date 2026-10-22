# CPA Rust HTTP/2 固定版本兼容补丁验证

状态：HPACK 补丁已实现并做隔离对照；完整 Rust 替代仍未完成，Go worker 保留。

## 本轮修复

用户批准维护固定版本 HTTP/2 兼容补丁后，引入 `crates/http2-go-compat`。
固定官方 `http2 0.5.20` 原始归档及摘要，仅在 Cargo OUT_DIR 生成补丁源码。
不修改全局缓存，不改源码行数治理基线。来源和摘要见该 crate README。

新增 Go 编码策略修复路径、Cookie、Authorization、Content-Length 等动态表
索引差异、重复字段值索引、等长 Huffman 选择及容量阈值差异。
未标记请求继续走原实现；兼容开关是内部请求扩展，不是可伪造的入站 HTTP 头。

第一次五轮运行仍出现重复头块 67 字节，追查确认上游 streams 在转换请求前
清空了扩展。接线改为仅保留无资源的兼容标记，其他扩展仍在锁外清理。
随后重新构建探针、重跑五轮，重复头块降到 12 字节。另增加真实内存 HTTP/2
连接测试，覆盖标记穿过发送队列及未标记行为，防止只测编码器导致假阳性。

上游 crate 发布时排除了 HPACK 夹具，初次完整测试因此出现路径 NotFound。
从 crate VCS 信息指定的同一 GitHub commit 取得夹具，固定摘要后解包到 OUT_DIR，
修复测试路径，而不是跳过这些测试。

## 实际对照范围和结果

隔离目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-cpa-rama-http-20261021`。
本地 TLS 服务端、显式追加临时 CA、合成认证字段，不关闭证书验证。
基线是 Go 1.26.1 标准 Transport 探针，对照是实际 Rust transport crate 探针。
**本轮不是完整 CPA EXE / Gateway EXE / 真实服务商三路验收。**

Go/Rust × HTTP/2/HTTP/1.1 共四组，每组五轮，均返回 200，每组复用一个连接。
证据为 `go-h2.json`、`rust-h2.json`、两份 HTTP/1.1 JSON、`report.json`、
`comparison.json`；`compare.py` 从原始 HEADERS 帧解码，不用会归并 Cookie 的事件结果。

八项断言通过：SETTINGS payload、连接 WINDOW_UPDATE、伪头顺序、头字段多重集、
动态表条目集合、稳态 12 字节头块、DATA payload、HTTP/1.1 序列化。
动态监听端口的 authority 被归一化；普通头按集合比较不是顺序比较。

原始五轮 HPACK 长度：Go `[133, 15, 12, 12, 12]`，Rust `[136, 12, 12, 12, 12]`。
首两轮不同与 SETTINGS 到达/ACK 调度、table-size update 放置有关，不能冒称原始字节一致。
Go 普通头顺序变化，Rust 当前固定，这也仍是行为分布差异。

## 验证门禁与待办

- HTTP/2 依赖完整单测与新增兼容测试；保留上游既有 ignored 项。
- transport 配置测试通过。
- `cargo check --lib --features cpa-rust-transport` 通过，仍有 24 个现有 warnings。
- 官方 rustfmt、有效行数 checker 31 项测试、ratchet、`git diff --check` 执行通过。
- 未执行完整 Gateway 测试、桌面类型检查、打包、发布、真实服务商新一轮请求。

删除 Go worker 的前置条件尚未满足，至少还要关闭以下边界：

1. HTTP/2 ACK/窗口调度、table-size update 时机及普通头顺序分布。
2. Gateway EXE 启动与真实服务商对照，不以库探针代替产品路径。
3. Rust transport 响应解压（HEAD/无正文、多 Content-Encoding、多 gzip member）、
   typed body-limit 分类、重复 Connection 清理、初始化 deadline/取消、H1 并发测试。
4. 敏感标记审查：当前 wire 层为了 Go 索引策略清除 sensitive 标记，必须在启用
   生产/trace 日志前改为既保留日志脱敏又能选择 HPACK 索引的内部策略。
5. HTTPS/SOCKS 代理、TLS HRR/额外组、官方域名不同传输路径和完整 CPA 身份模式。

本轮未提交、推送、部署、修改日常配置或删除 Go worker。

## 后续核验：请求头、解压与取消生命周期

用户要求继续后，新增失败测试后修复以下问题：

- 重复 Connection 字段：合并解析全部 token，不再漏转第二个字段指定的逐跳头。
- HEAD/204/304 和显式零长度响应不建立解压器，保留表示元数据。
- 多个 Content-Encoding 字段按出现顺序组成链，逆序解码；未知链整体保持原样。
  解码器嵌套限制为 8 层；保留 Gateway 自动解码语义，不假称 Go 响应 API 完全相同。
- gzip 多 member 不再只返回第一个 member。
- 敏感日志标记与 HPACK 索引分离：wire 不再清除 sensitive 标记，拆 Cookie 时
  继承标记；只有显式 Go 索引分支忽略该标记的 never-index 含义，Debug 仍脱敏。
  新测试同时检查压缩表索引及编码器 Debug 不出现合成 secret；默认分支测试不变。
- HTTP/1 等待响应头时超时/取消会在释放 origin 锁前关闭待定连接；不自动重放请求。
- HTTP/1 未消费完整正文就丢弃响应时关闭连接，正常 EOF 才允许复用。两个真实
  loopback 回归分别复现原来“下一次显式请求失败”，修复后均返回正确正文。

传输层 9 项测试通过；HPACK 完整测试重跑结果为 436 passed、0 failed、1 个上游
既有 ignored。官方 formatter、31 项行数 checker 测试、ratchet、diff check 均通过。
修改后的手写源码最大 198 有效行，没有改行数基线。新的合成五轮对照仍通过原来的
八项比较断言，四组共 20 请求全部 200。

另运行三种服务端 SETTINGS 延迟（0/20/100 ms）的 Go/Rust 各五轮测试，共 30
请求全部成功。在这六组样本中，两边初始帧类型均为 `[4,8,1,0,4]`，HPACK
长度均为 `[133,15,12,12,12]`。基准样本仍出现 Rust `[136,12,12,12,12]`。
这证明 ACK 和容量更新位置受调度影响，并非恒定编码差异；未为凑帧顺序加 sleep
或延迟生产代码，也没有证明两个实现的长期调度分布一致。
新增证据：隔离目录 `timing-report.json`、`timing-*.json`、`check_timing.py`。

上方原待办中的解压、重复 Connection、敏感标记、H1 取消/丢弃正文项目已关闭；
初始化 deadline、typed body-limit、H1 并发、H2 取消/子任务清理、代理、TLS HRR、
官方线路、身份模式和完整 Gateway/服务商验收仍未关闭。Go worker 保留。
