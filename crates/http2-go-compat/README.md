# 固定版本 HTTP/2 Go HPACK 兼容补丁

本 crate 替换 `http2 = 0.5.20`，不是完整 Go 网络栈。仅带有内部
`GoCompatibility` 请求扩展的头块选择兼容编码器；未标记请求仍使用原编码器。
扩展不是 HTTP 字段，不接受入站请求头选择。CPA 使用独立连接池，不能把不同
调用风格混用到同一 HPACK 连接状态中。

## 来源与审计

- 上游：`https://github.com/0x676e67/http2`
- 版本对应 commit：`5a9a1fe28154461318310e6044959c6dc8a60d17`
- 官方 crate：`upstream/http2-0.5.20.crate`
- crate SHA-256：`92d3114be2f413b2e491e686b93a28cda30c355cffc8d091a57f8be4b1342896`
- 原许可证：`upstream/LICENSE`，MIT；原归档保留作者和源码。
- crates.io 包排除了 HPACK 夹具，因此从同一 commit 下载源码，只提取
  `fixtures/hpack` 中 JSON 与 README 为 `upstream/hpack-fixtures.tar.gz`。
- 夹具归档 SHA-256：`307c762f4594ca79d61ecec39b9c2d8cc792e246a7af0e09a180e163d7b4a252`

`build.rs` 先校验不可变归档摘要，仅在 Cargo `OUT_DIR` 解包，并检查每个
接线替换的精确命中次数。构建不联网、不改 Cargo 全局缓存。手写补丁位于
`compat/` 和 `build.rs`，按正常源码行数规则检查；没有修改行数基线或排除策略。
夹具路径只在生成的测试源中指向 `OUT_DIR`。

## 行为边界

- Go 模式允许路径、Cookie 等原先被硬编码排除的字段进入动态表。
- 每个同名字段值分别查询和插入动态表。
- 只有严格缩短时才选择 Huffman；等长使用原始字符串。
- 动态表容量阈值使用完整容量；保留服务端重复容量设置的通知。
- 未标记请求的 sensitive 策略不变；显式 Go 模式按 Go 的索引策略处理这些字段，
  但保留 HeaderValue 的 sensitive 标记，以保护请求与动态表的 Debug 输出。
  该模式不能用于要求凭证永不进入压缩表的线路。调用方仍须独立审查日志脱敏和
  连接池隔离，不能把允许 HPACK 索引等同于允许日志输出凭证。
- 保留扩展时只保存无资源的兼容标记，仍在流锁外释放其他请求扩展，避免破坏
  上游的锁/资源生命周期约束。

补丁不解决 SETTINGS ACK 调度、普通头随机顺序、TLS HRR、代理覆盖等差异。
不能据此删除 Go worker，也不能宣称所有 CPA 路径或非随机网络字节一致。

## 验证

```powershell
cargo test --manifest-path crates/http2-go-compat/Cargo.toml --lib
```

2026-10-21 验证：436 passed，1 个上游既有 ignored，0 failed。
另以隔离自签 CA、合成凭证执行 Go 1.26.1 与 Rust 的 HTTP/2、HTTP/1.1
各五轮真实 socket 调用。原始帧显示稳态 HPACK 从 67 字节变为 12 字节，
与 Go 一致；SETTINGS ACK 时机仍不同。具体边界见 `docs/progress/cpa-rust-hpack-20261021.md`。
