# R07：Qwen Web 正常网页材料保存与精确账号测试

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。只新增 opt-in 测试宿主及父模块接线，没有修改生产实现或前端。本批不是 Qwen 整卡、P50/P60/P70 或真实供应商验收。

## 执行链与观察

真实 Edge 新建隔离测试页 → 正常 `main.tsx` Web 构建 → 透明同源回环代理 → 正常管理 TCP 登录/Grant/配置写入与 probe → 精确合成 Qwen 账号 → 非转发 v2 回环上游 → 本地 SQLite 审计及正常集中测试结果页面。

新增 `src/upstream/client/qwen_web_pipeline_tests/browser_host.rs` 复用现有配置 writer/CAS fixture 和 Qwen test-only task scope；scope 缺失不能回退真实平台。使用合成管理 token、合成 bearer/Cookie/UA，不读取或转发真实材料。前端构建只输出本批 `linshi/safe-web`，不执行现有 live 发布/清理 hook。

1. 在正常账号编辑器读取合成材料后取消：`material-cancel.json` 记录上游 0、revision 未变。这一观察来自第二版宿主；其后修正的是宿主 expected 文档，前端和生产链未变，没有把取消称为最终宿主重新执行。
2. 最终宿主新目录中，保存 `qwen-pipeline / exact` 的新 bearer、完整 Cookie 和原 UA；管理 verify/confirm-secret-access/route-config 均返回 200。`material-saved-final.json` 记录完整 typed 文档匹配、revision 已变、上游调用仍为 0。
3. 打开该精确账号的正常测试弹窗，取消继承、清空默认题集，只加入一个合成临时用例及合成标准答案；不保存策略，不启用定时测试。运行一次，仅一个管理 probe POST、一次 create、一次 completion。
4. 通过正常服务商测试入口切到 IQ 结果页，实际看到 `public-alias / exact`、`1 / 1 题正确`、合成测试指数 100；供应商额度及比值均为 `—`，页面明确没有用 token 或估算费用替代。这不是模型智力、真实质量或供应商额度证据。
5. 宿主检查实际出站新 bearer/Cookie/UA、native model `qwen3.7-plus`、唯一 completed 管理 probe 审计、usage 2+3=5、`accessKeyId=null`、精确账号归属及敏感材料/私有 prompt 未进入审计；完整配置比较保证其他账号未变。

正常编辑器既有语义也纳入精确预期：Qwen session auth 为 bearer，不是从 Cookie 文本推断 cookie transport；保存还补齐 `enabled`、`model_selection` 并重建所属 provider 的模型并集。宿主没有忽略整个配置或放宽生产准入来取得通过。

## 最终证据与检查

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-probe-ui-20261112`。

| 检查 | 最终结果 |
| --- | --- |
| Rust 编译 | `cargo test --locked --offline --lib --no-default-features --features runtime-defaults -j 4 --no-run --message-format=json`，最终 `build-v4` 退出 0 |
| 公共 pipeline / TCP | 6 passed；浏览器宿主按默认 opt-in 规则 ignored |
| 精确账号 probe 相邻 | 2 passed |
| 新增浏览器宿主显式执行 | 1 passed；去重合计 9 项，不重复计入中间构建测试 |
| 前端相关测试 | 4 文件、67 passed |
| 类型 / 隔离正常 main 构建 | 均通过，最终 UI 输入无漂移 |
| checker / ratchet / formatter | checker 31 passed；最终官方无报告 ratchet 和两文件 rustfmt 通过 |
| 规模 | 父模块 264→266，新宿主 169 有效行，无 baseline/exception 修改 |

Cargo 返回的实际测试 EXE 已复制后执行，最终 SHA256 为 `c50035eb29904686035b299687b1dc82cc9d18ae6415441bc202c683adadd9c3`，副本 `qwen-ui-v4-tests.exe`。这是测试 harness，不是应用 EXE 或 devtest 包。最终 `build-v4-inputs.json` 对当前选定 Rust/Cargo/build 输入无漂移，`ui-inputs.json` 对当前前端及 Neuro UI 输入也无漂移；不是所有源码测试全量通过的声明。

主要证据：`browser-http-v3.jsonl`、`material-saved-final.json`、`probe-observed-final.json`、`browser-result.txt`、`browser-result.jpg`、`browser-runtime-v3/completed.json`、`pipeline-v4.log`、`probe-v4.log`、`browser-host-v4.log`、`resource-final.json` 和最终回执。

管理服务 graceful shutdown/join、fixture SQLite 与合成 peer 关闭后才写完成回执。Node 代理通过本轮专用 stop 文件正常退出；最终宿主/代理进程及两监听端口均不存在。前两版宿主也通过 stop 分支退出，没有残留在线测试实例。

## 保留的失败与纠正

- 初次启动编译后修正测试预期，为避免使用旧源构建，仅核对并停止本轮直属 rustc，保留该次非零退出；不停止其他工作线的构建。
- 第一版 UI 显示只读：误复用 `with_runtime=false` 的公共请求 fixture。只在新宿主启用现有 writer/CAS，未修改生产授权或只读检查。
- 修 fixture 时发生 `LocalRuntime::open` 的 `expected &Path, found PathBuf` 编译错误；按真实签名借用路径，原错误保留。
- 第二版材料保存实际成功，但 `materialSaved=false`：expected 错把 Qwen 当 cookie transport，且漏了普通编辑器模型元数据。按 `providerSessionProducts.ts`、`providerSessionTarget.ts`、`credentialModelSelection.ts` 及合成保存结果修正精确预期；该版停止并标记未验证，不当作成功宿主。
- 最终第三个新运行目录由第四轮有效编译产物完成。上述均为测试夹具/预期问题，不冒称发现并修复了产品 bug；正常网页执行是必要验证，不能以只读审查替代。

## 剩余边界

本批证明正常材料写入/内存生效、新材料真实出站和结果页面联合，未重建 runtime 验证持久化重启恢复。未建立有余额普通 Key，不以 `accessKeyId=null` 冒称扣额/退款验证。未覆盖真实 TLS/WAF、signin/MFA/challenge、refresh、多轮/工具/模态、当前模型权益/额度、Server/真实 PostgreSQL、独立应用进程或包。

没有使用真实凭据、请求真实供应商、操作容器/在线实例、出包、提交或推送。新增模板仍 17，无新增整卡 `offline_ready`，总计划继续。后续不要再把本批已验的正常材料保存/精确 probe 网页链列作空白；可继续其他尚缺证据的渠道，或按独立任务推进运行时重启/Server/包边界，不猜未知 wire。
