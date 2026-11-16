# Suno BrowserBacked 精确音乐测试与正常网页联合

日期：2026-11-08。R15 的本执行子批已通过下述限定离线链；不表示整卡 `offline_ready`、P50/P60/P70 或总目标完成。

## 1. 改了哪些内容

承接[基本浏览器模型绑定](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-08-provider-channel-expansion-suno-model-binding.md)，删除正常媒体 probe 中“worker 尚未绑定模型”的旧 Suno 门禁；沿原 `resolve_execution_mode` → 普通 sender → 版本化 bound executor → 全部 MP3 验证链运行，不强制直连、不另起生成器。

正常账号表单允许合法 `direct_http / browser_backed`，保留精确账号、typed adapter、产品地址、配置模型/alias、草稿/revision/Grant 与逐次费用确认。不新增参数、保存计划或自动生成。

正常 main 网页联合实测另暴露共用会话目标判断误拒绝管理 wire 的 `base_url:null`：可选空值本应继承另一 owner 的固定产品地址。只将该处的 `!== undefined` 改为 `!= null`，最终地址仍必须存在且严格匹配；非空错误值、跨域、查询参数和双空值仍拒绝。新增14项覆盖全部8个会话产品、provider/credential 两种继承及拒绝边界；有效行60→60。

合成 Rust 合同覆盖 provider/endpoint/credential 三种浏览器配置、回执缺失/错模/pending/远端错误、任一 MP3 失败、executor/两首音频阶段 Drop、管理 Grant/Origin/费用与精确账号门禁。只退出本地等待，不宣称已撤销远端受理任务。

## 2. 真实红测与失败分类

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-browser-probe-20261108`。

- `red-ui`：真实 **3 failed / 14 passed**，三种 BrowserBacked 配置均被旧 UI 门禁禁用。
- `red-rust`：Windows `LNK1104`，原测试 EXE 被独立进程占用；这是链接环境失败，不算行为红测，未终止该进程。后续仅使用 Cargo `--config profile.test.package.gateway.debug=1` 区分根包调试信息/产物哈希，保持 runtime-defaults、锁定离线依赖，不修改仓库配置或删除缓存。
- `red-rust-r2`：真实 **0 passed / 1 failed**，得到旧 `Unsupported` 而非 `Passed`：`Suno browser music tests require native model binding, which is not yet available; generation was not sent.`；随后才删除后端门禁。
- 首次 `rust-green` 因独立 Vertex history 测试五个 `E0425` 未编译通过；外部 owner 补齐父模块测试 import 后才启动 `rust-green-r2`。本批未改 Vertex/Dim owner，未关闭测试或使用旧 EXE 绕过。
- 正常网页首次被上述 `base_url:null` 阻挡，`browser-blocked-null.json` 确认 **0 probe POST / 0上游调用**。随后 `red-ui-null` 真实 **8 failed / 6 passed**，修复后再构建独立 `safe-web-r2`；旧失败快照与构建均保留。

## 3. 最终运行与静态证据

`rust-green-r2` 同次编译前后选定输入稳定，立即冻结测试程序；SHA256：

`a11a9c92f24e2be3f933fd5a822d26f4ec1ffa4f2bd5a523777c11c606cb02d6`

准确来源见 `rust-green-r2.json` 和 `final-proof-r2.json`；失败编译、旧程序和后续共享 target 均不是本批凭据。

- `rust-focused`：15 passed / 1 ignored；`rust-contract`：358 passed / 6 ignored。后者包含前者，不相加。
- 显式运行新 opt-in 正常网页宿主 `ui-host`：1 passed。按同一 lib harness 完整名合并，最终 **359 passed / 0 failed / 5既有 opt-in ignored**，不是把 ignored 当通过。覆盖 Suno/MP3/browser executor、相邻媒体 probe/worker/Local pipeline、headers 与 lifecycle；未运行全仓测试。
- 最终 `ui-null-final`：14文件 **192 passed / 0 failed**；`typecheck-r2`、正常 `src/main.tsx` 的隔离 `ui-build-r2` 通过，962项前端/共享 UI 输入前后及收尾 hash 一致。原112项阶段是历史，不累加。未触碰共享 dist。
- 开启 Suno 的 `production-on-final` 与仅关闭 Suno leaf 的 `production-off` 均 exit 0；最终开启配置的选定输入稳定，关闭配置期间只变化两个前端文件，不影响其 Rust 消费输入。初次 `production-on` 也通过但存在并行输入变化，故以最终开启配置回执为准；不是全 feature 矩阵。
- 4个本批 Rust 文件的官方格式检查通过；checker 单测31/31通过，输入未变复用本批回执。最终 ratchet：**5128文件 / 14 immutable runtime artifacts / governed source >700=0**。本批8个源码/测试最大182有效行，未改 policy/baseline/exceptions。

## 4. 正常 main → 管理 TCP 联合

正常网页与新隔离浏览器，经过登录、真实 Grant、原生产 router 和真实回环 TCP，精确选择 `suno-pipeline / exact / public-suno-music`，提示词为合成固定材料，不制造管理授权或组件回调：

1. 打开当次费用确认，明确取消；`browser-cancel.json` 记录 **0 probe POST / 0上游调用**。
2. 再运行，重新确认费用；透明 proxy 只转发一次正常 probe POST。使用生产 bound remote sender，合成服务收到版本化入口与精确 native model/Cookie，随后两次匿名资产 GET，无 direct/legacy/replay 或另收费下载。
3. 正常页面显示“已验证2首MP3 / 供应商扣额未知”；DOM无 audio/video、产物链接或 IQ。`browser-confirmed.json`、正常页面截图/CLI记录、`browser-runtime-v1/completed.json` 保存证据。
4. Rust终态核对 requested/resolved/alias、精确账号、封闭未知费用审计、普通 Key 余额仍2、完整配置和 revision 不变；TCP退出并 join、SQLite关闭、原 worker shutdown、合成上游停止后才发布 completed。

该联合只证明 **正常 UI/管理 TCP/生产 bound remote 合同/真实 MP3 字节**；远端是合成服务，不是本批真实 Node/browser worker 或真实 Suno。前一子批的真实隔离 CDP 模型绑定证据单独保留，不能冒称此次全链真实官网生成。

自建浏览器已正常 close。第一只静态 helper 因 stdin 已关闭，仅核对身份后停止自建精确 PID；第二只以 tty 保持 stdin，正常 stop 并退出。`resource-final.json` 核对4个自建 PID及4个回环端口均无残留；未停止其他进程或现有实例。

## 5. 来源、范围与继续边界

本批13文件（8源码/测试、5文档）按直接 preimage 审计，不以 dirty HEAD 代交付源码。冻结 Rust 本批范围逐项匹配；`ui-host` 期间变化为本批前端空值修复及独立 Dim 测试 `src/protocol/dim_oauth/pipeline_tests/native.rs`。最终前端使用修复后独立构建，Rust无需因前端修改重编；冻结测试程序不包含后续独立 Dim 测试修改，不宣称最新全仓联合回归。完整选定输入、变化清单、检查与范围 diff 均在本批目录。

仅退役基本 BrowserBacked 精确音乐 probe/逐次费用确认及上述正常网页联合缺口。当前官网模型/feed wire、持久device/Clerk/受众/challenge/refresh、实际额度/下载权益、完整参数/其他音频格式、远端硬取消/lease、完整公共/Server/独立进程/原生包及 OAuth/MCP 业务仍继续。

模板仍17，无新增整卡 `offline_ready`，P50/P60/P70及总目标未完。无真实 `.ng`/凭据、真实生成或收费下载、Redis/Docker、现有实例修改、出包/提交/推送；不复用已消费的P34授权。
