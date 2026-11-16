# Gemini feature 关闭路径构建门禁窄修复

用户明确授权仅修复首批三个服务商独立构建中已复现的 S06/Gemini 编译门禁，不接管 Gemini 其他功能。

## 原因与改动

protocol 的 Web reverse surface 在 `line-gemini-web-reverse` 与
`line-gemini-canvas-program` 都关闭时使用 disabled 实现，但 upstream 仍编译需要完整
surface 的 execution、speech、transcription、uploads，造成 15 个缺失符号错误。

- `src/upstream/gemini/web_reverse/mod.rs` 对这些模块及导出使用与 protocol 相同的 feature 条件。
- 新 `disabled.rs` 保留四个执行函数的接口，统一返回既有
  `gateway_provider_line_compiled_out` 错误，并标明 `gemini_web_compatible`；不进行 I/O。
- 没有修改 feature 开启时的执行实现、协议类型或业务行为，没有通过强制开启 Gemini feature 绕过错误。
- 新集成测试 `tests/gemini_web_disabled_execution_contract.rs` 调用四个关闭入口并检查本地代理陷阱没有收到连接。

## 构建回执

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-first-three-20261113`。

统一使用 `cargo check --locked --offline --lib --bins --no-default-features -j 2`：

| feature | 日志 | 结果 |
| --- | --- | --- |
| `line-commandcode-provider-api` | `gate-commandcode.log` | passed |
| `line-sensenova-token-plan` | `gate-sensenova.log` | passed |
| `line-sap-aicore-chat` | `gate-sap.log` | passed |
| 不指定 feature | `gate-none.log` | passed |

构建曾等待其他工作线持有的共享 Cargo target 锁；没有杀进程、清缓存或重复启动同一检查。

关闭入口运行测试：`gate-disabled-contract.log`，**1 passed / 0 failed**，
编译 4m37s、运行 0.04s。四个入口均返回预期错误，本地代理陷阱未收到连接。命令：

```powershell
cargo test --locked --offline --no-default-features --test gemini_web_disabled_execution_contract --config profile.test.package.gateway.debug=0 -j 2 -- --test-threads=1
```

`gate-checker.log`：checker 31 项通过；`gate-ratchet.log`：ratchet 通过。
定向 rustfmt、scoped diff 与源码 UTF-8 无 BOM 检查通过；源码和本次 Command Code
测试 EXE 哈希记录在 `native-gate-source-hashes.json`。未改 baseline。

## 验收边界

本回执只覆盖授权的构建门禁，不证明 Gemini 真实上游行为，也不代表 Command Code、
SenseNova 或 SAP 产品收口。没有调用真实账号、改动运行实例或凭据、出包、部署、提交或推送。
