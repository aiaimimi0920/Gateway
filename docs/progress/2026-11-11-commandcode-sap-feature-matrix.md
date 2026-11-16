# Command Code / SAP 独立 feature 检查（2026-11-11）

状态：**独立裁剪仍受阻**。本轮补齐真实编译诊断和对照，不把开启额外 Gemini 功能后的成功当作独立配置通过。

## 验证范围

统一命令前缀：`cargo check --locked --offline --lib --bins --no-default-features`。复用共享 target，Cargo jobs=2；首轮等待其他会话构建锁，未终止他人任务。所有命令只做类型检查，不链接 EXE，不执行运行验收。

| 附加 features | 结果 | 说明 |
| --- | --- | --- |
| `line-commandcode-provider-api` | failed，exit 101 | Gemini Web 15 个符号缺失错误 |
| `line-sap-aicore-chat` | failed，exit 101 | 相同 15 个错误 |
| 无 | failed，exit 101 | 相同 15 个错误，证明并非只启用某一产品线才触发 |
| `line-commandcode-provider-api,line-gemini-web-reverse` | passed，exit 0 | 仅为诊断对照，不是独立 Command Code 配置 |
| `line-sap-aicore-chat,line-gemini-web-reverse` | passed，exit 0 | 仅为诊断对照，不是独立 SAP 配置 |

每轮前后核对 Cargo.toml、Cargo.lock、build.rs 和 src Rust 文件共 3119 个文件的 SHA-256，均无变化。日志有警告，成功行只表示 exit 0，不表示零警告。

## 已定位的共用阻碍

- `src/protocol/gemini/mod.rs:35-45` 在 Gemini Web 与 Canvas 都关闭时，将 `web_reverse` 指向 `web_reverse_disabled.rs`。
- `src/upstream/gemini/web_reverse/mod.rs:3,13-16` 仍无条件声明 execution、speech、transcription、uploads；对应执行实现引用了 disabled surface 未提供的符号。
- 具体错误包括 `surface::speech`、`speech_response`、`WebUpload`、`mime_and_extension`、`without_media_replay`、`prepare_media`、`validate_text_request`、`pack_request_with_uploaded_files`、`has_media`、`accumulate_complete_media_response` 和 `canonical_to_sse`。原始错误类型为 E0432、E0433、E0425。

下一责任动作：由保留的 S06/Gemini owner 对齐 feature 关闭时的执行模块门禁与 disabled surface；不能通过给 Command Code / SAP 强制添加 Gemini feature 依赖把裁剪失败隐藏起来。修复后再复跑前三个独立配置。本轮遵守 S06 写入边界，没有修改这些源码，也没有修改 Cargo feature 依赖。

## 未改变的结论

上轮默认配置下 63 项 Rust 与 15 项 UI 通过的回执仍保留，但不能替代本次裁剪配置。Command Code / SAP 的真实账号、费用、独立链接构建／运行、Server／Redis、自动调度器到期执行、原生包与完整产品验收仍开放。没有读取真实凭据、部署、重启、提交或推送。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-commandcode-sap-features-20261111`。含每个配置的原始日志、命令、退出码、源码快照及 `comparison.json`。
