# Luma Agents 正常 UI、真实管理 TCP 与本地持久化重建

日期：2026-11-05。状态：本子批定向验证通过；不是 R14/W10、P50 或总计划完成。
仓库：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。
承接 [正常模板/Key UI 子批](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-05-provider-channel-expansion-luma-agents-console.md)，本批不修改生产源码或新增模板。

## 1. 实施与存储边界

新增三个 integration-test owner，通过现有公共 API 使用生产 `build_app_state`、真实 loopback TCP router 和 local bootstrap；没有扩大生产可见性、添加生产控制端点或上游目标覆盖。

- [自动合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/luma_agents_local_contract.rs)：168 有效行。
- [HTTP/runtime owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/luma_agents_local_contract/runtime.rs)：148 有效行。
- [显式 opt-in 浏览器 host](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/tests/luma_agents_local_contract/browser_fixture.rs)：74 有效行。

两类持久化严格区分：供应商配置/Key 使用既有 Console revision 文件与配置事务 owner；普通公共接入 Key、密封秘密和余额使用现有 `runtime.sqlite3` owner。不把供应商 Key 保存说成写入 SQLite，不手工修改 SQL 或 storage version。
Redis pool 关闭、无 PG、无公共 fallback Key；未启动后台 scheduler、FreeBuff 任务或任何上游生成。
默认测试线程栈、同任务 boxed bootstrap；没有增大线程栈或设置 `RUST_MIN_STACK`。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105`。
测试数据、TEMP/TMP、控制文件、构建、截图与日志均在该目录。仅创建全新 fixture 根，不读取 `.ng`/历史凭据、不覆盖在线数据、不删除已有目录。

## 2. 自动真实 TCP 合同

1. 正常 SecretPatch Replace 无 grant 为 403，错误 Origin 为 403；revision 不变。
2. 正常授权后保存合成 Luma Key 为 200，管理响应不回显材料；复用旧 revision 保存为 409。
3. 模型 metadata 与 account metadata 编辑沿 Keep，保持原 Key、稳定 provider/credential ID、固定 Agents base、Bearer/adapter 和无 refresh 身份。
4. 正常管理 API 创建普通公共 Key 并充值 4 条；没有发送任何公共生成请求。
5. await HTTP graceful shutdown 与真实 SQLite close，随后同目录/同端口重建；原 revision、供应商 Key/ID 保留，普通接入 Key 可由正常 POST secret-copy 取回原 token，余额仍为 4。
6. 擦除 Luma 产品并改目标后的 Keep 返回 400/`secret_keep_destination_changed`，active revision 不变。
7. 同 Host/Origin 的旧 grant 在重建后清空为 403；取得 fresh grant 后 Clear 为 200，再次重建仍为空 Key，账号身份及余额 4 保留。

这证明保存/读取/重建与无生成时余额不变，不证明 Luma 公共媒体请求的扣费、退款或 settlement。

## 3. 正常浏览器 UI 结果

使用正常 `main.tsx` 的全新隔离 Rsbuild，透明代理只转发已有 API：不改 Host、Origin、auth 或 body，不制造 grant，不加载发布/删除 hook，不改工作区 dist。

1. 正常登录、session verify 和 confirm-secret-access；从正常目录创建 `lumalabs-api` 空池。
2. 正常账号编辑器录入合成 `ui-luma`/Key/`uni-1`，通过真实管理 PUT 保存。
3. 正常编辑器只增加 `ray-3.2`，没有触碰 Key，沿 metadata Keep 保存。
4. 第一次同目录/同端口 runtime 重建，后端直接断言 Key 与身份不变、revision 为 `r3-df37f1d3fcf3`、模型为 `uni-1`/`ray-3.2`；页面 reload 仍显示账号和两模型。
5. 正常“显示 API Key”触发真实 `credential-api-key` 200，随后明确清空并保存；没有读取或绕过浏览器的秘密脱敏显示。
6. 第二次重建，后端断言 Key 为空、revision 为 `r4-1466a8e2b832`、身份与两模型保留。页面仍显示未验证模型和请求数 0；不增加验收标记、不据空表单推断秘密状态。

浏览器只使用本地合成材料；不调用真实 Luma，不点击普通文本测试自动制造付费媒体生成。

![清空并重建后的正常账号库](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105/ui-after-clear-rebuild.png)

## 4. 验证、失败分类与来源

- 最终自动 TCP：`local-tcp-v4`，1 passed / 0 failed / 0 ignored；编译 16.07s，测试 0.54s。
- 最终浏览器：`browser-ui-v2`，显式 `--ignored` 实际运行，1 passed / 0 failed / 0 ignored，402.22s；不是把默认跳过算通过。
- 本批共 2 个不同 Rust 测试通过。生产库/二进制及新的 integration target 由实际 Cargo 命令编译；没有重跑前批 238 项或前端 382 项冒充本批新增。
- 两个成功阶段各冻结 3020 项选定 Rust/配置/manifest/test 输入，运行期间及最终核对无漂移；首轮编译后仅三个新测试文件变化，没有借测试修复修改生产源码。
- 全新正常 main 隔离构建通过，0.80s，15 个实际产物（含摘要未列出的附属文件）；884 项选定 UI/锁文件/实际安装 `@neuro/ui` 输入最终无漂移。前端生产源码未改，不重复此前无变化的类型与单元测试。
- 三文件官方 Rustfmt、checker 31 项、ratchet、完整 `git diff --check` 通过。ratchet 扫描 4519 文件，4505 governed source 没有 >700；14 个 immutable runtime artifacts 分报，不变更 baseline/exception。
- 七份本批源码/文档的 UTF-8 无 BOM、含未跟踪文件的 scoped whitespace 与来源核对通过。真实 ratchet JSON 从仓库根 `artifacts/` 保存，不读取旧 `scripts/artifacts/` 报告充数。

失败现场全部保留：v1 因测试调用 private snapshot API 编译失败，改用已有公共编译产物读取，不扩大生产 API；v2 错用 GET 调用现有 POST secret-copy 端点，产生空的非 JSON 响应，仅修正测试 method，原日志没有记录 HTTP status；浏览器首轮要求 UI 不存在的 account-name 字段而失败，仅改为核对实际模型 metadata，自动合同仍单独编辑 account metadata。
浏览器自动化中失效的 AX 引用/不匹配 DOM role 以及模板保存尚未结束就开表单均通过刷新实际 UI 状态解决，没有用注入 callback 或直接管理写入绕开表单。不是生产缺陷红绿测试，也不把失败记录改写为成功。

回执：[最终核对](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105/closeout.json)、[浏览器 host 完成](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105/browser-runtime-v2/completed.json)、[自动阶段](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105/local-tcp-v4.json)、[浏览器阶段](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105/browser-ui-v2.json)、[资源退出](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-luma-local-ui-20261105/resource-audit.json)。

## 5. 退出与未完成边界

最终 host 先 await HTTP/SQLite 关闭再写 completed receipt；原 Cargo 会话 exit 0，Node 代理经 stdin stop 正常 exit 0，专用标签关闭，无 viewport 改动。
两轮已知 PID 33780/48720/50532/首轮 host 和 61283/58179/57771/52961 端口均无残留；没有停止任何其他服务或创建 Redis/Docker。P34 历史许可已消费清理，本批不复用。

本批只有同进程 AppState/runtime 重建，不是原生 EXE/OS 重启，也不涵盖 scheduler、PG/Redis/Hosted 或整包验收。
Luma 精确媒体 probe 仍需显式操作/费用授权合同；公共媒体 router/普通 Key/route/usage/audit/本地结算/失败取消仍未闭合，后续继续这些真实缺口。
其他官方操作、HDR+10s 文档矛盾、视频真实播放、旧 Web，以及原定其他渠道/OAuth 兼容恢复/Qwen 受限项和 P50 全范围无凭据集成/devtest、P60 集中录制、P70 实际模型/权益/费用/额度全部保留。
正常前端新增模板累计仍 16，不增加整卡 `offline_ready`；不出 EXE 包、不提交推送、不替换在线实例，不将本子批当总目标完成。
