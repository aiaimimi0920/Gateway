# Browser executor request helper split

本批继续处理 Gateway 中尚未有专属 owner 记录的 501–700 行文件。

## 变更

- `src/upstream/browser_executor_helpers.rs` 保留 runtime health、远端响应解析和服务响应构造；请求输入读取、header 组装、endpoint key、错误格式化和执行状态映射迁移到新的 `browser_executor_request_helpers.rs`。
- 父模块通过 `pub(crate) use` 保留原有导入路径，调用方和错误/协议字段不变。
- 新 owner 只有请求边界的纯转换逻辑，没有新增 I/O、锁、task、retry 或资源生命周期。

## 行数与门禁

- effective-line checker：`2262 files; >1500=10, 701-1500=18, 501-700=21`；本批将 browser executor helper 从 501–700 清单移除，未新增超限文件。
- `cargo fmt --all -- src/upstream/browser_executor_helpers.rs src/upstream/browser_executor_request_helpers.rs` 及针对这两个文件的 `cargo fmt --check` 通过。
- `npm run check:effective-lines --prefix scripts` 通过，ratchet 无 violations。
- `git diff --check` 通过。

## 未完成验证

`cargo test --locked upstream::browser_executor_helpers -- --test-threads=1` 在共享 Windows 编译窗口超过 90 秒，仅停留在依赖编译阶段，手动终止；没有将 Rust test/compile 记为通过。未运行真实 browser executor、远程 provider 或 release 构建。
