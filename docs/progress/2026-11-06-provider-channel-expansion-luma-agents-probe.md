# Luma Agents 精确账号媒体测试与费用确认

日期：2026-11-06。状态：本子批定向验证通过；不是 R14/W10、P50 或总计划完成。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。
承接 [公共媒体 pipeline 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-06-provider-channel-expansion-luma-agents-pipeline.md)。
证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106`。

## 1. 范围与实现

复用现有 `POST /v1/internal/gateway/console/providers/{providerId}/probe`，不新增绕开鉴权的操作端点。原 management auth、精确 Secret Grant/Origin、账号筛选、revision 和轮次预算 owner 保持不变。

新增可选 `media` 请求：`operation=image|image_edit|video`、`confirmCharge=true`、可选 `parameters`；必须显式型号和单个精确 `credentialIds`。不得与 scope、临时或命名文本计划混用，parameters 不得覆盖 model/prompt/type/n/stream/response_format。
产品参数经过原媒体 normalizer 和 Luma mapper 准入，再走原 JSON passthrough sender；一次 POST、同 UUID 轮询、下载和验证资产，不重提、不回退、不发现其他账号。

原普通文本测试、保存策略、临时/命名计划和自动文本执行不获得媒体费用授权，不会隐式生成。媒体结果没有 `answer`/文本 assessment，不把 generation ID 当答案评分。

审计复用原 `ProbeAudit` 的取消安全存储及健康状态 CAS，分别保留 requested model/alias、resolved model、精确账号和本次操作/费用确认；成功时共享公共 pipeline 的 native 白名单，只保留 UUID/model/type/state/credits，不保存输入图像、资产 URL 或 Key。null/0/小数 credits 不进入 Token 或 USD 字段。
此管理测试不使用普通公共接入 Key，不对其余额扣费；不证明供应商不扣费。本地取消不承诺远端撤销或退款。

## 2. 正常账号 UI

实际 `PilotActionDialog` 只对独立 `lumalabs-api` 的账号单点入口显示媒体操作表单；其他产品和池级文本计划维持原入口。
支持图片生成、带 URL/generation/file/有界 inline Base64 source 的编辑、视频的显式时长/分辨率。只列出当前账号已配置、匹配操作的实际型号；草稿未生效、账号停用/不存在或目标/型号不匹配时不准入。
每次运行都重新确认可能的供应商扣额，以及本地取消不等于远端撤销/退款；不保存费用确认或自动策略。配置身份变化使待确认操作失效，关闭沿原请求 owner 取消等待。
复用现有 `@neuro/ui`、样式类、控制器及请求/取消 owner；没有新增颜色、独立设置或解释型页面。

## 3. 验证记录

- UI red-v1：0 passed / 2 failed，暴露缺少正常媒体操作/提示词入口；随后 green-v2 的 8 项新测试通过。
- UI 相邻回归：6 个文件 31 项通过，原请求 owner 另 15 项通过。前端 typecheck-v2 exit 0。
- 正常 `main.tsx` 隔离构建通过，实测 1.24s；不导入仓库发布/删除 hook，不修改工作区 dist，不出 EXE。
- Rust red-v1：0 passed / 3 failed；其中一项是 media 合同未实现，两项是夹具误用复合账号 ID，已修为真实 `exact`，并合并到原 Luma test module，避免两套 task-local remap。
- Rust green-v1：新增测试的 `Option<u16>` / `Option<i32>` 断言类型不匹配，编译失败，仅修正测试。
- Rust green-v2：取消已通过，HTTP 失败终态触发 `STATUS_STACK_OVERFLOW`；单独运行成功路径也复现。没有加大栈或设置 `RUST_MIN_STACK`。去掉审计完成的重复异步包装，并在原健康完成责任边界 heap-pin 大 future，不增加线程/任务或改变取消所有权。
- 最终默认栈 `green-v3`：41 passed / 0 failed / 1 ignored，编译实测 16m58s，测试 43.09s；共享 model-probe 相邻 20 项通过，测试 8.33s。没有重复累计前批相邻 53 项。
- 浏览器 host 另用显式 `--ignored` 实际运行：1 passed / 0 failed / 0 ignored，265.02s。与上方按名去重共 62 个不同 Rust 测试通过，其中本批新增 10 项（9 项自动合同和 1 项显式浏览器 host）。
- 前端按名去重 46 项通过（本批新增 8 项）；887 项选定正常 main/锁文件/实际安装 `@neuro/ui` 输入由隔离构建冻结。Rust 成功阶段各冻结 3031 项选定输入，阶段回执无漂移；最终联合来源审计见本批 closeout receipt。
- 生产 `runtime-defaults` 和代表性关闭 `line-gemini-web-reverse` 库检查通过。Rustfmt、checker 31 项、ratchet、scoped UTF-8/无 BOM/空白检查通过。20 项受影响源码最大 399 有效行；原 recording 从 160 到 207，console-model-probe 从 388 到 399，无 baseline/exception 改动。ratchet 扫描 4533 项，14 项 immutable runtime artifacts 分报，governed source 没有 >700；保留根 artifacts 的原字节报告。

## 4. 正常页面到管理 TCP 的证据

使用正常 `main.tsx`，全新生产 Local bootstrap、真实管理 loopback TCP、SQLite 和原合成上游 fixture。test-only middleware 为每个 HTTP 任务显式绑定原 remap，不改变生产目的地，不放宽 auth/Grant/Origin，不增加测试专用生产 API。
透明静态代理保留浏览器 Host/Origin/auth/body；实际 session verify 和 confirm-secret-access 均为 200。
正常账号库进入 `exact` 媒体表单后，输入提示词，首次费用确认选择取消；此时 HTTP 日志只有 verify/grant，没有 probe。再次运行重新出现确认，确认后仅一个 provider probe 200。
页面实际显示 `uni-1`、原 UUID、`credits_charged=null` 与未知语义；没有文本 IQ 结果或资产 URL。后端独立断言一次 POST→同 UUID GET→PNG GET、精确 Key/账号、资产无鉴权、native 审计和普通 Key 余额仍为 2。

![正常媒体测试结果](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106/ui-result.jpg)

浏览器无观察到的 JS error；两次 CDP 点击回执超时后均先刷新实际 UI 状态，已确认操作实际生效，没有盲目重发生成或注入 callback。关闭专用标签页，没有调整 viewport。
后端先 await HTTP graceful stop、SQLite 和上游 fixture 退出再写 completed receipt，Cargo exit 0。静态代理会话 stdin 已关闭，无法发送原 stdin stop；核对新 PID 65272 和精确脚本路径后仅停止此 helper，退出码 1 单独保留，不冒称 graceful exit 0。最终 PID 25184/65272 和端口 65301/63339 均无残留。

关键回执：[最终联合核对](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106/closeout.json)、[默认栈绿测](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106/green-v3.json)、[浏览器 host 完成](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106/browser-runtime-v1/completed.json)、[管理 HTTP](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106/browser-http.jsonl)、[资源退出](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-probe-20261106/resource-audit.json)。

所有失败现场保留。没有真实凭据、真实平台、付费上游或容器操作；P34 许可已消费清理，未复用。

## 5. 未完成边界

本子批只覆盖显式单账号媒体测试，不扩为后台媒体生成计划、跨进程任务恢复或完整供应商费用结算。
Luma 其他操作、晚到 credits/cash、视频真实播放、Web 独立渠道及实际材料/权益验收继续；Server/Redis/PG/Hosted 及原生 EXE 不能由 Local/合成 fixture 外推。
原定其他渠道、OAuth 兼容恢复、Qwen 受限项和 P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道真实验收均保留。
正常前端新增模板累计仍为 16；不增加整卡 `offline_ready`，不提交、推送、部署或替换运行实例。
