# ChatGPT Web 当前账号真实验收与 r39 手测包来源

## 本轮真实调用

2026-11-01（Asia/Shanghai）恢复并检查现有 r38 桌面实例。后端 PID 3460、桌面 PID
41736，连接 `http://127.0.0.1:4200`；`healthz`、`readyz` 均为 200。另一个监听地址
的实例未操作。没有停止、重启、升级或替换当前实例，也没有修改路由或重新导入凭据。

通过正常账号库的“测试账号”入口，对 `chatgpt-web` 中已有模型的指定账号（标识以
`vmjcv333` 开头）执行一次临时手动测试。取消继承、清空全模型选择后，只选择已配置的
Web 模型 `gpt-5-6` 和一个用例；未保存策略、未开启自动测试、未批量测试其他账号。

```text
prompt: Reply with only OK.
model: gpt-5-6
answer: OK
status: passed
checkedAt: 2026-10-31T22:20:59.7882036Z
elapsedMs: 4389
completedCount: 1
correctCount: 1
```

生产探测 owner 为 `src/provider_runtime/chatgpt_web_model_probe.rs`，沿当前有效调用流程
进入真实 HTTP 执行器，固定账号和模型；没有路由 fallback、隐藏重试或浏览器 recovery。
该 owner 只有在非空文本且 `finish_reason=stop` 时才返回 passed。没有把 Web 模型别名
成 Codex OAuth 模型。结果从 `credential_test_results` 精确只读核对并存为脱敏回执；
没有读取凭据表、导出浏览器状态或保存 Cookie／token。

此前 10/31 的 bootstrap 403 历史继续保留。本轮成功证明当前选定账号／模型的文本
HTTP 调用可用，不证明另一账号、全部模型、多模态、长期会话刷新或所有网络出口可用。
本轮没有另外执行真实公共 API 普通／SSE 调用；这些入口本轮只做本地合同回归，不能
将本次管理 UI 生产探测当作对所有公共入口的真实验收。

## 改动

- 在 `providerTemplateAdaptation.ts` 登记 `chatgpt-web` 的新流程成功证据，显示蓝色及
  可访问的“新调用流程已验收”标识；与 `chatgpt`（Codex OAuth）保持独立。
- 更新目录测试和使用文档，不因目录可读、健康检查或旧 canary 自动登记。
- 本轮没有修改 ChatGPT Web 执行器；真实测试已通过，没有人为添加绕过挑战的代码。
- 新包同时包含此前尚未编入 r38 的三家 Google Web 专属 JSON 导入；其离线解析及
  保存链证据见 `2026-11-01-google-web-json-import.md`，不扩大为真实新 JSON 登录验收。

## 本轮验证

- 蓝标回归先确认缺少 receipt 的红灯，随后登记并修正测试中的实际模板排序后转绿。
- 前端 12 个定向测试文件、92 项通过；TypeScript 检查通过。
- ChatGPT 定向 Rust lib 测试 230 项通过；3838 项过滤，不是全量测试。
- Rust HTTP 合同 6 项通过：精确账号生产探测、公共流式终态／失败审计、Google JSON
  管理鉴权／revision／对象存储边界。均为 loopback 合同，不是新的真实上游调用。
- Node Cookie 导入／wire／runtime 打包合同 9 项通过。
- `cargo fmt --all -- --check`、checker 自测 31 项、effective-line ratchet、
  `git diff --check` 通过；未改 baseline、阈值或例外。扫描的 governed source >700 为 0，
  14 个 immutable runtime artifacts 仍单独计量。
- 本轮变更的 effective lines：receipt 文件 131 → 136，目录测试 118 → 122。
  新改源码和文档使用 UTF-8，无 BOM。未扩写运行时状态、凭据或数据库 schema。

Windows PowerShell 首次验证包装器把普通 native stderr warning 作为 terminating
error 中断。只修正临时包装器的错误流处理后重新运行 Rust 及后续检查，以进程退出码
和最终测试摘要判定通过；没有隐藏编译 warnings 或将中断记为通过。

## 打包与交付边界

使用官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4` 构建独立
`gateway-devtest-20261101-web-json-chatgpt-r39`，不覆盖旧包、不携带 `.ng` 或登录材料。
打包前重新核对可用内存约 13.7 GiB；构建串行，保持默认 4 个 Cargo jobs。显式纳入
已经校验的 AI Studio proof runtime，仍由源码绑定、固定版本、许可证和文件 hash 门禁
确认有效，不复制历史 Gateway EXE 冒充新构建。

最终构建、包完整性、隔离原生启动结果和 EXE SHA256 由包内 manifest／构建凭据及下列
目录的 `package-result.json`、`packaged-smoke.safe.json` 记录，不以本文计划当作通过：

```text
C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-chatgpt-web-acceptance-20261101-r39
```

隔离启动使用该目录下独立数据根及随机 loopback 端口，无已有账号和凭据，不发上游请求；
短暂创建的测试进程结束后只清理本次自有进程。当前 r38 实例及其数据保持不变。
本轮未提交、推送，未执行正式发布安全审计或全量／付费多模态回归。
