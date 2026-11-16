# R07：Qwen 正常网页保存、宿主进程重启与恢复后调用

日期：2026-11-12（Asia/Shanghai）。工作区 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway`；HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。仅修改 opt-in 测试宿主及新增 local_owner.rs，没有生产或前端改动。

## 同一轮跨进程执行

正常 main.tsx/Edge → 管理 TCP/Grant/CAS → 生产 new_local 文件 authority 和本地 SQLite → 首宿主退出 → 新 OS 进程从同根恢复 → 正常网页精确账号 probe → 合成 Qwen create/completion → 结果页面与脱敏审计。

1. **保存进程 PID 24192**：正常编辑 qwen-pipeline/exact，导入合成网页 token、完整 Cookie 与 UA。Qwen session_auth 按 bearer/authorization 解释，不从 Cookie 推断 cookie transport。完整配置匹配、其他账号不变，保存 revision 为 `r1-d982cd4f021a`，上游与审计均为 0。
2. save-exit 使首宿主 graceful shutdown/join、关闭 SQLite/peer 后写 saved.json 并正常退出；代理也退出。外部确认旧进程和两个监听消失后才继续。
3. **恢复进程 PID 31648**：显式 GATEWAY_QWEN_PROBE_UI_RESUME=1，从同根生产 authority 恢复；必须有 saved 回执、不同PID、同revision、无已有 completed 回执。浏览器 probe 前完整材料已匹配，上游仍为0，没有重新导入或保存。
4. 新服务网页重新登录，选择 exact/public-alias，取消继承、清空默认题集，仅加入一个临时合成题；只运行一次，不保存策略或启用自动测试。全链只有首进程一次配置PUT和第二进程一次probe POST。
5. 实际只有一次 create 与一次 completion，没有重放；两请求使用恢复后的 bearer、完整 Cookie、原UA，completion native model 为 qwen3.7-plus。唯一 completed 审计归属 exact，consoleModelProbe=true，accessKeyId=null，合成上游 usage 为 prompt=2/completion=3/total=5，敏感材料和私有prompt不进入审计。
6. 正常集中IQ页显示 public-alias/exact、1/1题正确、合成指数100；供应商额度及比值保持 `—`，不拿已知tokens替代未知扣额。100仅表示合成答案匹配。

此处是同一次保存材料跨真实测试进程退出/启动后成功调用，不是仅重建对象或拼接历史独立测试。两个进程使用同一冻结测试EXE，但不是成品gateway.exe/gateway-ui.exe；AppState/auth仍由test support构造，未执行完整build_app_state和全部后台worker。合成wire缺task scope不会回退真实网络。

## 代码与检查

- browser_host.rs 改用真实本地authority并支持save-exit/resume，有效行169→199；新增 local_owner.rs 54有效行。默认仍ignored，普通首启动要求全新绝对目录。
- locked/offline、runtime-defaults、4 jobs 的lib构建退出0。
- 公共pipeline/TCP 6 passed；管理qwen_web_probe_tests 2 passed。宿主保存/恢复两次进程执行各1 passed，不能把同名测试计成两个新增测试。
- 首次管理过滤器误写成 qwen_web_model_probe_tests，实际0 tests，未计通过；核对真实模块后改为qwen_web_probe_tests，probe-final.log记录2 passed，原probe.log保留。
- 正常main隔离构建通过；前端输入与已验67项测试/类型检查版本一致，最终也无漂移，不重复未变化检查。
- checker31 passed、官方无报告ratchet、rustfmt和限定whitespace通过；无baseline/exception修改，文本UTF-8无BOM。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-qwen-process-ui-20261112`。Cargo compiler-artifact 返回的真实EXE复制核对后执行，tests.exe SHA256：`e8c042d4646441a10cbe73bba5e94d523c50c850ef8bde64b1a6ddab6b70913a`。

最终全src清单仅 `src/token_refresh/tests/nous_probe/inflight.rs` 存在并行漂移；Qwen直接输入与前端未变。结果绑定冻结副本，不宣称其他工作线当前修改全验。

主要证据：material-saved.json、first-process-exited.json、ready-save.json、ready-resumed.json、material-restored-before-probe.json、browser-http.jsonl、browser-result.txt/jpg、browser-runtime-v1/saved.json、browser-runtime-v1/completed.json、host-save.log、host-resume.log、resource-final.json。两个宿主及两个代理正常退出，四监听无残留；TEMP/TMP和保留的合成配置/归档/SQLite均位于本批linshi。

## 剩余边界

Qwen正常网页保存→真实local→测试进程重启→恢复材料实际调用的限定空白已退役。Grok同等级证据见前批；二者均未覆盖成品EXE/全启动/包、硬杀或断电、真实平台登录/MFA/challenge/refresh/TLS/WAF、真实账号模型权益/扣额、多轮/工具/模态、Server/PG或普通余额Key。

新增模板仍17，无新增整卡offline_ready，P50/P60/P70未完成。未用真实凭据、调用供应商、修改在线实例、出包、提交或推送。
