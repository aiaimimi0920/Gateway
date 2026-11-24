# R14：Luma LLM Inference 独立文本渠道

日期：2026-11-19（Asia/Shanghai）。承接用户选择“先完成最近 Luma，再按渠道扩展推进”。本批接官方独立 LLM 产品，与[媒体分层／视频 UI 收尾](2026-11-19-provider-channel-expansion-luma-video-controls.md)分开记账；R14 整卡与 Magpie 67 项仍未整体关闭。

长期回执目录：[本批归档](../evidence/luma/gateway-provider-resume-20261119/verification.json)。下文日志及 JSON 文件名均相对此目录；今后运行使用[仓库工具](../../tools/luma/README.md)，不使用旧临时脚本或固定哈希测试 EXE。

## 官方合同与实现

匿名 GET [官方指南](https://docs.agents.lumalabs.ai/guides/llm-inference/) 返回 200；保存正文、UTC、最终 URL 和 SHA-256，见 `../evidence/luma/gateway-provider-resume-20261119/sources.json`。指南明确独立销售开通 Key、固定 inference 域名、token 后付费，不属于 Agents 图片／视频 Key 或 credits。

增加 `lumalabs-inference-api` preset、`lumalabs_inference` profile、`luma_inference_key` auth mode、独立 Cargo line 与 manifest。模板默认空池、无模型／路由、关闭自动测试、不配置健康证明。正常账号表单手工录入模型和独立静态 Key，拒绝已知媒体 Key、外部目标或会话／发现材料；metadata Keep、明确清空与暂存 replacement 走原编辑／SecretPatch／revision owner。

暂存 Key 在最终提交时复核产品和固定目标。`credentialDocument.ts` 修改前已为 500 有效行，故把既有四种静态 Key 的 staging dispatch 连同新增产品放到单责模块 `credentialKeyStaging.ts`，保留预览略过无效材料、真正提交报错的既有语义；不新增第二套秘密保存机制。

共享调用链支持 Chat、无状态 Responses／Messages 的 Chat 桥接，JSON／SSE 和原生 function tools。固定上游 `/v1/chat/completions`；禁止进入 native Codex transport，拒绝服务端状态、媒体输入及 hosted tools。最终 sender 清除 ambient／caller 认证头、只发精确账号 Key、不跟随重定向。普通和原生 JSON 入口均使用有界 Chat 准入；SSE 使用原严格 Chat completion guard。无效生成不能假成功或重放，原用量审计、预付次数结算和取消清理继续复用。

默认 probe 不发网；显式模型测试经现有精确账号／grant 边界和同一 sender，要求完整短文本、无工具／refusal，结果脱敏后写原审计和健康 owner。没有实现未经公开合同支持的 models／余额接口。

## 验证记录

- 首个独立 preset 测试曾在未实现时真实失败；实现后修正配置测试误用 `RouteConfigStore::new(document)` 的夹具错误，改用实际 `from_document`，未放宽生产准入。
- 首轮 Rust 定向 `luma_inference` 16 passed / 0 failed：真实生产 Local bootstrap、公共 router、管理 API 创建普通预付 Key、真实 loopback HTTP 和 SQLite；覆盖三公共入口 JSON／SSE、工具两轮、账号与 token 审计、扣次退款、错误／截断／取消、默认头隔离及不重放。见 `luma-inference-rust.log`。该日志保留为首轮证据，最终增量验证另记。
- 前端相关 12 文件共 108 个不同测试最终通过。扩大并发时两项视频控件测试触及 5 秒超时；同文件单 worker 复验 5 passed，未改超时配置、断言或生产行为。账号 staging 相邻 Qwen／Kiro／Vertex 三文件 26 passed，计入前述 108 项。
- TypeScript 通过；实际 main 入口隔离生产 Web build 通过，保留正常入口／配置，移除发布插件且关闭清理，输出仅在临时 `web-main`，不清空既有 `dist`。
- manifest validator 55 份通过；checker 31 passed；effective-line ratchet 通过（5828 measured files、14 项 immutable runtime artifacts，governed source >700 为 0）。

最终增量 `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 luma_inference --no-fail-fast` 为 **19 passed / 0 failed**，包含管理 grant、禁用／错误模型不发网、显式／命名／账号策略测试与 feature 启用验证，见 `luma-inference-final.log/json`。同一最终测试 EXE 的 implementation-line 相邻回归 **38 passed**，含上述 feature 启用用例，不重复相加。生产 `runtime-defaults` lib check 通过，保留 115 项既有 warnings，见 `luma-inference-prod.log/json`。

不启用 Luma line 的代表性 `cargo check --locked --offline --lib --no-default-features --features family-openai-compatible-official-api -j 4` 通过（191 项该 feature 组合下的 warnings）；这证明关闭组合可编译，不冒称已运行关闭组合测试 EXE。

本轮 36 个 Rust 文件的 `rustfmt --edition 2021 --check --config skip_children=true` 与 `git diff --check` 通过；源码范围 UTF-8 无 BOM，最高 483 有效行，`credentialDocument.ts` 500→483。最终 65 个范围文件的 SHA-256、编码与计数在 `scope-source-hashes.json`，汇总在 `verification.json`；这是含继承修改的范围快照，不声称全部 dirty 工作均由本轮编写，不以首轮 EXE 代替最终验证。

## 未验边界与后续

公共验证使用 router oneshot 到真实本地 HTTP 对端，不是正常前端到管理 TCP 的联合验收。真实销售账号、模型资格、上游限速／费用、Server／PG／Redis、跨进程与原生包没有验收；本轮没有真实推理／扣费、提交／推送、重启实例或覆盖旧包。

Luma 后续仍包括 Files 上传／管理、MCP 创作生成／取消／结果／费用、Web 当前 wire／模型绑定、媒体取消期间尚未上交 receipt、截止后／跨进程追账／cash，以及真实资产／播放与完整交付。它们继续列在[剩余清单](../providers/magpie-remaining-2026-11-13.md)和[渠道合同](../provider-luma-channel-contract.md)，不以新文本模板关闭整卡。
