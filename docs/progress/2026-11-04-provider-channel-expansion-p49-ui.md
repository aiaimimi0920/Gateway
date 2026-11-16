# P49 后续增量：Kiro 管理 UI / API client

日期：2026-11-04（Asia/Shanghai）。本批完成已有 `kiro` preset 的管理入口接线与定向离线验证，
承接 [后端管理合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-04-provider-channel-expansion-p49-console.md)。
不是全计划 P50，不发布新的 Kiro 模板，不把 R02/W02 标为整卡 `offline_ready`。

## 1. 改了哪些内容

- 独立 `kiro-auth.ts` 定义真实后端的 directory/scopes、registering/waiting_user/polling 等状态及
  challenge/receipt schema。使用共享管理 transport 的原 token、Secret Grant、30 秒预算、binding
  核对和编码后的 session ID；GET 不带 secret grant/body，不制造 callback URL 或第二套 API。
- 共享 action 新增 `poll` 传输能力；`useManagedOAuth` 只在设备流策略和 waiting_user 下准入，
  Kiro 不接受 callback `complete`。原 Accio/Grok 的回调路径与策略保留，并有相关回归覆盖。
- 定时 1.5 秒检查仍是串行 GET，不发送设备轮询 POST。操作员明确点击“检查授权”才推进一次设备流；
  后端承担 pacing/pending/slow_down。重复并发、registering/polling/ready/终态不再发送新 poll。
- 表单不预填目录、scope 或 machine，不内置借用 client ID。Builder ID 与 IdC 二选一；IdC 显式
  Directory Region/Start URL，不能从业务 base/profile Region 推断。五个已知 Kiro scope 显式勾选，
  并不宣称某组是当前最小 scope。新账号 machine 显式输入，重授权不提供新设备覆盖入口。
- `ConsoleGlobalDialogs` 按精确 preset 分派，而不是显示名称或候选 ID；绑定已保存 revision，
  dirty、locked、非 active revision 禁止新修改。新增/稳定 ID 重授权走服务端普通 SecretPatch/CAS；
  不将 OAuth 回执送入通用 API Key 表单。
- Kiro 普通编辑保留 metadata/模型/enabled/keep，不提供独立 bearer/API Key 替换字段；
  reauthentication-only 的 submit guard 也拒绝旧/程序性 replace、clear 或通用新增绕过。
  这不是 Kiro 原生 CLI Key 实现，也不静默迁移无 typed context 的旧账号。
- 只有 succeeded 才关闭并 refresh。ready 不自动保存；unknown/save_unconfirmed 不创建第二账号、
  不重新提交、不能绕回普通表单；允许关闭并在当前 Console 挂载期重开查同一 receipt。
  Map 按 client/管理身份独立，不保存 token/callback，不写 localStorage；仍非跨应用重启恢复。
- 可执行链接仅允许 HTTPS AWS device 根/固定 Builder Start，拒绝任意 URL、用户信息、端口、
  fragment、错误 path 和伪造域名。这里只证明导航目的地可信，实际 directory Region 绑定仍由后端校验。
- 复用 `@neuro/ui` 的 Field/Input/Select/CheckboxGroup/Button/Notice 和既有 Dialog，未改全局主题。
  只有当前主动作强调；保存或登录成功不代表 profile、订阅和模型权益已验证。

## 2. 测试与失败记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-kiro-console-ui-20261104`。

| 检查 | 实际结果 |
| --- | --- |
| 行为红测 | `red-entry.log`：1 failed，真实 GlobalDialogs 找不到 Kiro 登录按钮；不是缺模块编译错误 |
| 初版入口 | `green-entry-v1.log`：8 passed；首次 typecheck 发现新增测试 mock 只有 2 参数却读取第 3 项的 TS2493，仅补正确 options 参数 |
| 定向与相邻回归 | 最终 `regression-final.log` / JSON：**26 文件、228 passed / 0 failed / 0 skipped**，其中本批新增 **65 passed** |
| 类型 | `typecheck-final.log`，exit 0 |
| 隔离构建 | `safe-build-v2.log`，exit 0；真实 `src/main.tsx` 与真实 GlobalDialogs/面板夹具一起构建 |
| 静态 | Rustfmt、ratchet、`git diff --check`、18 项前端源码摘要/UTF-8 无 BOM/有效行核对均通过 |
| checker | 复用未变化的后端证据目录 `checker-tests.log`，31 passed；未重复执行或改 baseline/exception |

前端无仓库配置的 formatter 命令/配置，沿相邻 TS/TSX 风格并执行类型/文本/diff 检查，
不临时安装 formatter，也不把 Rustfmt 当作 TS 格式验证。前端 18 项来源中 7 项既有、11 项新增；
最大 GlobalDialogs 为 328→340，hook 168→169，普通表单 hook 259→261；其他项均不超过 500。
冻结只覆盖本批源码，不是整个脏工作树的发行 provenance。

## 3. 浏览器实测与清理

隔离 builder 不加载仓库 staged publish 插件，`cleanDistPath=false`，产物只写 linshi，
不清空/改写工作区 `dist/web`。真实 GlobalDialogs → CredentialDialog → Kiro 面板，只有管理 API
使用明确标识的合成材料；未打开真实授权页面，不将此夹具称为真实管理服务端或原生应用 E2E。

Edge 实际验证了显式目录/scope/machine、challenge/用户码、GET 不推进授权、一次显式 poll、
ready 不自动保存、一次保存成功关闭/refresh、稳定 ID 重授权、save_unconfirmed 锁定返回编辑，
以及 Escape 焦点回到原 opener、关闭重开仍保留同一未知保存且不追加 POST。

初次尺寸设置受现有浏览器缩放影响，实际为 853/260 CSS 像素，未按请求值冒报；
用临时 CDP metrics 后读取真实 `innerWidth/innerHeight`，最终 **1280×900 和 390×844 CSS 像素**。
窄屏 pageWidth=390，dialog 宽/scrollWidth 均 356，滚动区宽/scrollWidth 均 298；
宽屏 pageWidth=1280，dialog 两者均 798，滚动区两者均 740。使用共享 CheckboxGroup 移除
初版原生 fieldset 的 min-content 边框布局；最终目标尺寸无横向溢出，有内部纵向滚动。
`visual-observations.json` 与 `wide-device-challenge.jpg`、`wide-ready.jpg`、
`narrow-save-unconfirmed.jpg` 保留实际结果。没有新增持续动画，未修改既有 reduced-motion 策略。

临时 server 只绑定 `127.0.0.1:55123`。核对完整脚本路径后只停止本批 Node PID 32388，
`server-cleanup.json` 记录剩余监听 0；专用标签已关闭，viewport/CDP override 已恢复。
没有读取 `.ng`/历史凭据、调用真实 OAuth/推理、创建 Redis 容器、修改在线实例、出包、提交或推送。

## 4. 接力边界

- 旧 route/未变 source hash 的 DB 自动升级仍缺；P49 folder 转换和普通 metadata keep 不等于迁移。
- 当前最小 scope、真实 Builder verification URI、profile 发现/选择与业务 Region/ARN 绑定、
  订阅/模型权益 exact HTTP、当前产品/client/额度仍须按 Kiro 调查合同推进，之后再发布正常模板。
- 原生 Key 已证但未实现；固定许可 direct HTTP 参考仍未采用，CLI/ACP 不能借用户已有会话。
- 跨 Console/应用进程重启的完整管理 receipt 恢复仍缺；当前 Map 不作持久恢复承诺。
- 模板保持 12 个；Vertex OAuth/Express、Luma Agents、各 Web/媒体生命周期、Grok 余项、
  Qwen OAuth 受限待决及 P50/P60/P70 均保持原范围，不因本批 UI 子链通过而删减。
