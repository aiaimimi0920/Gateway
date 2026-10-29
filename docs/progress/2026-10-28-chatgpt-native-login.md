# ChatGPT Web 原生两阶段登录续接

## 范围与当前状态

续接会话 `01a17b6c-200c-7b62-94d2-58620a056e23` 的无扩展网页登录与会话交接任务。
没有修改运行中的 4200 配置、凭据或数据库，没有关闭或重启 Gateway 后台及未响应的桌面壳。

结论分层：两阶段登录的真账号隔离实验与指定模型双模式调用通过；新路线已接回源码。
用户随后授权修补并交付桌面版 devtest；本文件记录冻结源码时的改动与已有证据。
新包的构建、完整性、取消和原生启动结果以文末所列独立交付回执为准，不能将旧 EXE 实验替代新包 UI 验收。

## 原因与路线

旧 Playwright Chrome/Edge 登录均被 Google 拒绝，用户实际看到“此浏览器或应用可能不安全”。
普通 `browser-cookie3==0.20.1` 读取日常 Chrome 曾返回 `Unable to get key for cookie decryption`。
没有继续重复这两条失败路线，没有安装导出扩展、注入浏览器、使用 stealth 或关闭浏览器保护。

新的流程：

1. 正常启动已安装 Chrome/Edge，仅使用本次新建的独立 profile，登录时没有调试或自动化参数。
2. 用户自行完成登录与 MFA，看到 ChatGPT 已登录首页后关闭专用窗口。
3. 同一份专用 profile 才进入 loopback CDP 阶段；不读取或接管日常浏览器 profile。
4. 精确读取 ChatGPT 网页会话，经过既有管理鉴权、secret grant、revision 和 secret patch 所有者保存。

本轮前段已联网读取的官方资料：

- Google 支持说明：<https://support.google.com/accounts/answer/7675428?hl=en>。
  软件自动化控制或内嵌浏览器可能被拒绝。
- Chrome 调试说明：<https://developer.chrome.com/blog/remote-debugging-port>。
  默认日常数据目录不能作为新的远程调试目标，应隔离使用非默认目录。

## 真账号实验，不等于新 EXE 验收

使用已有 devtest 后端及独立数据目录，经真实 Rust 管理接口完成 capture、持久保存和网关重启读回。
指定服务商/账号为 `chatgpt-web` / `web-native-login`，仅调用 `gpt-5.4`，没有遍历目录模型。

| 调用 | HTTP | 文本 | 结束检查 | 单次耗时 |
| --- | --- | --- | --- | --- |
| 非流式 | 200 | OK | terminal 有效 | 5875ms |
| 流式 | 200 | OK | terminal 与 `[DONE]` 有效 | 6984ms |

临时测试 Key 已撤销，隔离测试网关及专用浏览器已退出。以上不是性能承诺，也不证明过期后自动续期。

## 源码改动

- `scripts/chatgpt-web-session/browser-login-process.mjs`：自有临时 profile 和子进程生命周期；登录阶段无调试，
  关窗后使用动态 loopback 端口，校验 `DevToolsActivePort` 与 browser WebSocket nonce 精确一致。
- `scripts/chatgpt-web-session/browser-login.mjs`：两阶段顺序、唯一 ChatGPT 页面、固定可重试错误、
  总工作时限 205 秒；在 worker 220 秒硬停止前为正常清理预留 15 秒。
- `CredentialDialog.tsx`：网页登录材料继续走 Web 凭据所有者，不混入 Codex OAuth 或模型发现。
- `ChatgptWebSessionImport.tsx`：网页登录与 JSON 录入互斥；切换清除旧材料并卸载旧所有者，
  旧浏览器请求被取消，迟到的浏览器结果和文件读取结果均不能覆盖新选择。
- 登录状态提示要求登录首页后关窗；JSON 备用入口不再要求安装会话导出扩展。

没有新增依赖或改动锁文件，没有撤销工作树内的其他改动。

## 打包前的补充修复

- `console_browser_workspace.rs`：Rust 父级为控制台 browser worker 创建独立随机临时目录，
  通过 `TEMP`、`TMP`、`TMPDIR` 限定其 profile 所属范围，不改变普通 producer worker。
  工作目录先在阻塞任务中完成分配与所有权转移；监督任务在 HTTP future 取消后继续负责进程和目录。
- `producer_browser_worker_process.rs`：完成或强制取消后，先停止自有进程树，再有界重试移除自有目录；
  早退通过 `Drop` 异步回收兜底，根目录被替换时拒绝跟随，保留邻居目录。
  这不承诺操作系统崩溃、整个 Gateway 被强杀或运行时已退出时也一定完成清理。
- `chatgpt-browser-runtime.ps1` / `verify-browser-runtime.mjs`：只补入已锁定的 `playwright-core` 完整包，
  对照 manifest、lock 和已安装包版本，拒绝链接或未实现的依赖闭包；从 staging 实际解析并加载，
  所有文件进入既有 manifest/SHA256。未复制整个 `node_modules`，也未做 npm tarball 来源证明。
  原生登录入口本身只用 Node 内建模块；此修复解决后续常规 Web runtime 的交付依赖，不冒称旧 Google 登录错误的根因。

## 实际验证

- TypeScript：通过。
- 前端定向测试：4 个文件、27 项通过、1 项跳过；跳过项要求显式提供私有 worker 样本，未设置该输入。
- Node 登录、原生进程所有者、会话读取和 worker 协议测试：18/18 通过。
- 官方有效行计数器测试：31/31 通过；ratchet 通过；`git diff --check` 通过。
- 源码真实 Chrome 生命周期：本地静态测试页，不需要账号或 MFA；正常关窗后动态端口/nonce 校验通过，
  自有根进程退出、临时 profile 移除；另一次预期超时也完成同样清理。
- 首次 ChatGPT 页面生命周期试跑未在时限内关窗，返回 `browser_timeout`；进程/profile 清理通过，
  未进入调试采集。该失败回执保留，没有算作登录或采集成功。
- 本次涉及的 9 个源码/测试文件均通过 UTF-8 无 BOM 检查；最大文件为 `CredentialDialog.tsx`，
  有效行 390 → 392；新增原生进程模块 105 行，新增互斥组件 32 行，没有触及大文件豁免。
- 当前 scripts/desktop 没有官方 JavaScript/TypeScript formatter 入口或配置；没有伪报 formatter 已执行。
- 补充回归覆盖 Rust 临时目录唯一性、环境隔离、邻居保留、早退回收和强制取消回收；
  包测试覆盖完整复制及 hash、缺失/错版依赖和新增未支持依赖的拒绝。Rust 文件已运行官方 rustfmt。
  本轮 Rust 进程所有者有效行 422 → 495，新 workspace 模块 81 行、测试 52 行，均未越过限制。

## 未完成的交付边界

1. 新包真实账号的完整原生 UI 登录、MFA、关窗采集和保存留给用户手测；模板保持未标蓝。
   自动化取消验收采用无账号的自有假浏览器和持锁子进程，不重复用户认证或真实上游调用。
2. 父级强制取消回收已实现；实际测试结论见本轮回执。不得将正常/超时/取消清理外推为所有崩溃路径都已清理。
3. 网页会话失效后的长期无人值守续期未证明；本次不将会话读取、保活或双模式调用登记为自动续期成功。
4. 当前 Gateway 桌面壳未响应属于另一问题；保持原现场，没有用重启掩盖。

## 证据

目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-web-resume-20261028/`。

- `native-login-20261028-180444/result.json`：真账号 capture、保存、重启读回及进程停止回执。
- 同目录 `relay-result.json`：两次真实调用及测试 Key 撤销回执；原调用 guard 不得删除或重放。
- `source-lifecycle-result.json`：首次生命周期试跑超时的失败回执。
- `source-lifecycle-blank-result.json`：源码原生浏览器动态端口、nonce、正常关闭和超时清理回执。
- `source-effective-before.json`、`source-effective-after.json`：本轮源码有效行与编码检查。

真实 profile 和隔离 gateway-data 含会话秘密，不得打印、提交、上传或作为公开附件分发。

补充修复与交付证据目录：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-native-package-20261028/`。
其中 `effective-final.json` 记录补充源码的计数/编码；`cancel-*/result.json` 记录新包真实管理接口强制取消、
自有浏览器/子进程终止和临时目录移除；构建日志及交付回执分别保留构建退出码和实际产物哈希。
新包使用独立版本目录，不覆盖旧包；隔离验证使用 4300 与 linshi 数据，不接管正在服务的 4200。
