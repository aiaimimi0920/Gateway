# 2026-10-01 Windows 候选公开 HTTP 路由闭环

## 完成范围

直接启动已交付的 `gateway-wreq-20261001-windows-candidate/gateway.exe`，
通过公开 HTTP socket 验证鉴权、路由、协议转换、上游传输、余额和进程重启。
不是 `Router::oneshot()` 或直接调用 `stage_send` 的替代性证明。

本次 **19/19 用例通过**，没有修改生产源码、重新构建或覆盖候选包。
这是 Windows 本地候选的 HTTP/1.1 loopback 验收，不是完整 HTTP 迁移或公开发布批准。
原交付与手测入口见 [Windows 候选交付](2026-10-01-http-windows-release.md)。

## 首次失败与根因

保留 `r1/result.json`：首个 Chat 请求超时，成功用例和目标上游记录均为 0。
启动后的隔离配置快照证明 provider 地址正确，没有误载工作仓库的私密 routes。

夹具原先用 `NO_PROXY=*`，却继续继承机器代理。诊断改用本地代理捕获器后，
收到目标为 loopback 的 absolute-form 请求，而目标上游没有收到请求。
本地已安装的旧 `rquest-5.1.0/src/proxy.rs:672-686` 与新
`wreq-0.16.1/src/proxy/matcher.rs:439-446` 均按 IP/域名分支匹配：
`*` 属于域名规则，不匹配这里的字面 IP。这不是本次迁移新增的行为差异。

因此只修正测试夹具：使用显式 `NO_PROXY=127.0.0.1`，并把子进程的大小写
HTTP/HTTPS/ALL 代理全部指向不转发的本地捕获器。没有改变机器代理或产品策略。
`diagnosis-r1` 的首次诊断另因缺少创建 key 必填字段 `displayName` 得到 422；
补齐后 `diagnosis-r2` 才取得上述代理证据。两个诊断结果均保留。

`r1` 中固定的 `live_provider_calls=0` 不是网络测量结果，不能据其宣称测试
从未经过机器代理；该请求只有合成凭据和合成内容，配置目标也仅为 loopback。
有效的隔离与成功证据以 `r2` 为准：代理捕获器连接数 0，全部 17 个上游请求
实际到达本地夹具。

## 实际结果

| 用例 | 实测结果 |
| --- | --- |
| Chat Completions、Completions、Messages、Responses、Gemini，各普通响应和 SSE | 10/10；全部 200，有预期文本和 request-id，每次恰好一个上游请求 |
| 协议桥接 | 五类入口的上游路径均为 `/v1/chat/completions`；没有改成原生 Responses 上游掩盖桥接问题 |
| usage 与余额 | 普通响应 input/prompt 7、output/completion 3；10 次成功逐次扣 1，余额 20 → 10 |
| 无效 key | 401，上游请求 0 |
| 模型权限不足 | 403，上游请求 0，余额不变 |
| 上游 503 | 现有策略共尝试 3 次，最终 503，预扣余额退回 |
| Chat、Messages、Gemini 截断流 | 3/3；先收到合法文本，再收到 `IncompleteRead`；没有各自成功终态，且各只调用上游一次 |
| 客户端取消 | 首帧后关闭响应/socket，上游在 8 秒期限内检测断开，没有重放 |
| 余额耗尽 | 429、`message_balance_exhausted`，上游请求 0 |
| 重启 | 两次正常 drain 退出 0，SQLite 重启后余额仍为 0 |

10 次成功 SSE/普通响应的上游 Authorization 均为合成 provider 凭据，而非入口
access key。上游未观察到 management header；本矩阵没有向公开请求注入该 header，
因此不把这项观察冒充“注入后被剥离”的安全测试。

## 完整性与清理

- 执行前、执行后及最终审计均逐 hash 核对候选的 858 个文件。
- `gateway.exe` SHA-256：
  `06cab028a92dcd236d868d8071f15600f9858e7ec1f6e009b77f31dcee90822d`。
- `gateway-ui.exe` SHA-256：
  `6523c84a103865e5787bc212e07e304f23849a28b0bfa32dc6ddd1b64253ebe9`。
- 对照上一交付的 `input-preservation.json`，3,793 个既有工作输入无修改、无缺失。
  本检查点是随后新增的文档，不反向修改冻结的包或历史回执。
- 进程审计没有残留 Gateway/Gateway UI；测试端口 58108、58109、58110 均关闭。
  未停止无关 Python 进程。
- 测试脚本、数据、日志和诊断全部在 `linshi/gateway-wreq-router-20261001`。
  合成请求体上限 64 KiB、响应上限 256 KiB、请求记录上限 64，取消流最多 100 帧。
- `packaged_router.py` 有效行数 221 → 236，`upstream_fixture.py` 99 → 102；
  一次性诊断和审计分别 43、62 行。均通过 Python 语法检查和 UTF-8 无 BOM 检查，
  没有超过 500 行的新代码。计数由 Python AST/tokenize 排除注释和文档字符串。
- 本轮仅增加仓库状态文档，复用源码未变的编译、formatter 和行数门禁证据；
  重新执行 Gateway `git diff --check`。Neuro 根与 Gateway 原有 dirty 状态保留。

## 证据与复验

```text
C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-wreq-router-20261001
  r1/result.json
  r1/packaged_router.input.py
  r1/upstream_fixture.input.py
  diagnosis-r1/result.json
  diagnosis-r2/result.json
  r2/result.json
  r2/audit.json
```

后续如需复验，必须使用新的 label，脚本拒绝覆盖已有运行目录：

```powershell
rtk proxy python C:\Users\Public\nas_home\AI\GameEditor\linshi\gateway-wreq-router-20261001\packaged_router.py r3
```

本轮没有执行 r3。未重跑无关完整 suite，未提交、推送、部署、调用真实供应商、
导入真实凭据或删除 `linshi` 外内容。完整证书握手、HTTP/2 SETTINGS、真实提供方、
Linux/Docker/macOS 及公开发布安全门禁仍未验收；不能由此处的 19 项推导为全部通过。
