# Vertex Express 正常 UI、真实本地管理 TCP 与持久化重建子批

日期：2026-11-05。状态：本子批定向本地验证通过；不是 R11/W07、P50 或总计划完成。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。
承接 [公共 pipeline 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-vertex-express-pipeline.md)，不重复用 router oneshot 或 Console 保存 mock 证明真实 TCP/UI。

## 1. 实施边界

- 新增 test-only host：生产 `build_app_state` / local 配置 owner / SQLite、真实 TCP router 与 loopback 上游。
- 正常管理 API 创建/充值普通预付费 Key、Secret Grant/SecretPatch/原 revision/CAS；不手写 DB、不用 FakeRedisBackend。
- 正常 `main.tsx` 隔离构建及透明 API proxy；不伪造 grant，不修改 Host/Origin/auth/body，不运行原发布/清理 hook 或工作区 dist。
- 原 Express plan/header 检查之后才沿既有 `cfg(test)` 接缝换 loopback transport；不增加普通 EXE 的目标覆盖选项。
- 默认测试线程栈、同任务 boxed future，不复制 Kiro fixture 的 16 MiB runner，不设置 `RUST_MIN_STACK`。
- 同目录、同端口重建 AppState/runtime；这是测试进程内重建，不是真实 EXE/OS 重启，也不包含通用后台 scheduler 验收。
- HTTP graceful shutdown、SQLite finalizer/close 与 FreeBuff、上游 owner 先关闭并 await，再写浏览器完成 receipt。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105`。
保留写前 dirty edits；不提交、推送、重置或清理，不读取真实 `.ng`/历史凭据，不调用真实 Google 或创建 Redis/Docker。

## 2. 真实 UI 发现的缺陷与最小修复

正常 UI 保存空池后，真实管理 API 将未配置的 `adapter` / `protocol_profile` 等可选字段序列化为 `null`。
前端 `vertexExpress.ts` 只接受 `undefined`，导致正常添加账号误报固定地址/原生调用流程错误，尚未发出账号保存请求。
此前仅用缺省字段的模拟对象没有覆盖此问题；本批没有通过 API 绕开账号表单。

先补两项真实 JSON 形状的回归，均按预期红测；仅将可选字段的未设置判断改为 nullish，保留所有非空 override 限制。
另补五项非空非法 adapter/profile/flow 拒绝测试；固定 origin、产品、auth mode 和 Key 受众规则不变。
扩展测试时一度将用例放入 `setup`，Vitest 正确报告嵌套 test 错误；仅修正测试位置，原失败日志保留，不改生产语义。
前端源码从正常 `main.tsx` 重新隔离构建，页面 reload 后继续原空池和原后端 host；未重放公共请求或创建第二个 host。

## 3. 实际业务结果

1. 正常登录流程完成 session verify/confirm-secret-access；没有人工注入 grant，也不宣称出现独立二次确认弹窗。
2. 正常目录添加 `vertex-express-api` 空池，通过正常账号编辑器录入合成 `ui-key` / Express Key / `gemini-contract`。
   正常 SecretPatch/CAS 保存后实查 revision `r2-66f296e5d410`，管理响应不回显合成 Key。
3. 正常管理 API 创建普通公共 Key、充值 4 条；真实 TCP 公共 JSON 返回 `local-answer`，SQLite 余额 4→3。
4. 正常编辑器只增加 `gemini-metadata`，Key 输入未修改、保持 Keep；保存到 `r3-6726453e5030`。
5. 第一次同目录、同端口重建 runtime，后端断言原 Key 和 revision 保留；页面 reload 后账号 ID 和两模型仍在。
   复用原公共 token 的 SSE 返回答案和 `[DONE]`，持久余额 3→2。
6. 正常页面点击敏感读取入口，HTTP `credential-api-key` 返回 200，再明确清空并保存到 `r4-5b7ce793549b`。
   浏览器只读 DOM 的合成 Key 相等比较返回 false，未将其作为材料正确或错误的独立证据；没有尝试绕过浏览器敏感值处理。
   第二次同目录、同端口重建后，后端直接断言 Key 为空、revision 不变；页面仍保留账号和两模型。
7. 清空后的公共请求返回 400，余额仍为 2；host 关闭前核上游总数严格为 2，不增加失败后的调用或扣费。
8. 每次上游请求均检查精确 publisher-model path / `alt=sse`、`x-goog-api-key` 与 prompt；没有 Bearer、Cookie 或 `x-goog-user-project`。

自动 TCP 合同另证无 grant 保存为 403、stale revision 为 409、相同 Host/Origin 下旧 grant 在重建后为 403；
fresh grant 清空及后续重建的空 Key 保留。正常 UI reload 会由原登录流程重新取得权限，不冒称直接复用了旧 grant。

## 4. 验证与来源

- `local-tcp-v1`：1 passed / 0 failed / 0 ignored；编译 7m23s，测试 0.66s，默认线程栈。
- `browser-ui`：显式 opt-in `--ignored` 实际运行，1 passed / 0 failed / 0 ignored，580.49s；不把默认跳过计作通过。
- 本批 Rust 去重 2 项全通过；没有因 test-only 接线无故重跑前批 165 项或代表性 feature-off，也不累计前批数量。
- 前端定向 3 文件 27 passed / 0 failed，含 7 项新增 null/非空 override 回归；`tsc --noEmit` 通过。
- 修复后正常入口隔离 Rsbuild 通过（1.11s）；原构建凭据保留为 `ui-inputs-v1.json` / `ui-build-v1.json`，不拿旧产物冒充新源码。
- 四份 Rust owner 的官方 Rustfmt 定向检查通过；checker 31 项此前通过且输入未变，复用该成绩。
  最终官方 ratchet 扫描 4384 文件，14 项 immutable runtime artifacts，`governed source >700=0`；没有覆盖原有 dirty 报告。
- 六份源码有效行：`client.rs` 481→483、自动合同 191、runtime 152、浏览器 host 86、`vertexExpress.ts` 50→50、前端回归 71→85。
- 最终 2911 项 Rust 输入与两个成功阶段冻结 SHA 一致；修复后 UI 的 858 项源码/锁文件/实际 `@neuro/ui` 输入亦无变化。
  这是所选验证输入的来源核对，不声称整个 dirty worktree 冻结。
- `git diff --check`、十份本批源码/文档的含未跟踪文件 scoped whitespace、UTF-8 无 BOM 与本地链接检查通过。
  `git diff --no-index --check` 的内容差异 exit 1 未误当失败；临时计数命令的括号语法错误已改用文件脚本，不影响 checker 结论。

逐文件复核了 test-only 网络接缝、真实鉴权/CAS/秘密回显、固定产品目标、默认栈与 bounded HTTP/SQLite 关闭；
前端改动只接受后端合法的 null 形状，未放宽非空目标。临时工具与证据均在本批 linshi 目录。

证据：[最终回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105/closeout.json)、
[浏览器完成回执](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105/browser-runtime/completed.json)、
[JSON](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105/before-result.json)、
[SSE](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105/after_restart-result.json)、
[清空后](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105/after_clear-result.json)。

页面截图只证明账号库、两模型和已保留的调用数，不单凭圆点或空表单推断秘密是否存在；Key 清空结论来自后端断言及公共结果。

![重建后的正常账号库](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-vertex-express-local-ui-20261105/ui-after-clear-restart.jpg)

## 5. 资源退出与下一步

Rust host 先 await HTTP、SQLite、FreeBuff 和合成上游关闭，再写成功 receipt，原 Cargo session exit 0。
专用浏览器标签已关闭；本次 Node TTY host 接收 `stop` 正常退出，exit 0。最终核对 PID 32612/47420 与端口 60595/63528 均无残留。
未停止任何其他服务，未创建或清理 Redis/Docker；历史授权没有复用。

这是同一测试进程内 AppState/runtime 重建，不是原生 EXE/OS 重启，不包括通用后台 scheduler、PG/Redis/Hosted 或整包验收。
完整 decoder 的工具/schema、signed reasoning、阻断、多 choice，Full Vertex OAuth 独立 client/context/登录/持久化 refresh，
其余原定渠道及 P50 全范围无凭据集成/devtest、P60 集中录制、P70 逐渠道真实模型/额度仍未完成。
累计模板仍 14，不新增整卡 `offline_ready`；不读取历史凭据或真实 `.ng`、不调用真实 Google、不出包、不提交推送。
