# Gemini 新来源手测包与二进制运行验收（r15）

## 本轮关闭的剩余项

本轮没有继续扩展生产行为，完成此前缺少的“新源码 EXE、包内运行依赖、隔离原生启动和
真实 TCP 公共 API 合同”闭环。r11 默认 Web 媒体接线、r12 原文朗读、r13 图片只读恢复、
r14 加密认证 checkpoint 已进入同一个新的 desktop devtest 包；不再以旧 r10 包代替交付。

新包目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-dual-mode-r15/`。

使用官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4`，没有升级依赖、拷贝
旧 EXE、修改正式发布流程或覆盖既有版本目录。构建和打包合计实测 678.329 秒；这是复用
已有 devtest 缓存的本次结果，不是冷构建基准或提速承诺。原有编译告警仍存在。

## 来源与完整性

- `buildKind=devtest`，`edition=desktop`，源码 revision
  `fd74db9c374d8caa8309686592a0908c70376773`，包含当时的 dirty worktree。
- 构建前后及打包来源指纹一致：
  `5ecf72184e60385817d98a99135e1e733e2c2b97edd74469be63bb656621488f`。
- 构建凭据 `builtAt=2026-10-30T17:21:55.6667741+00:00`；profile `devtest`、jobs 4、
  opt-level 1、incremental 开启、LTO 关闭，默认端口 4200、数据目录 `.ng-devtest-4300`。
- 官方打包和最终发布目录 artifact smoke 均通过。额外逐文件重算清单中 5903 项的字节数和
  SHA-256；连同 manifest/checksums，目录实测 5905 文件、132312051 字节，无便携 `.ng`、
  无 `scripts/tests`。七个关键包内 worker 模块与仓库源文件 hash 一致。

| 文件 | 字节数 | SHA-256 |
| --- | ---: | --- |
| `gateway.exe` | 82171392 | `81324560adcbc2a40191d6fd30657f420d2d90be0d6f27e606474cb2a24c81ae` |
| `gateway-ui.exe` | 12862464 | `51758cb58d830c5886b9eb484022783bbcfb95681129090f79d2274c0848a808` |

本进度文档在打包完成后新增，不在该 immutable 包中；不因此宣称当前工作树的整体指纹仍
等于构建快照。没有在包内打补丁或写测试状态。

## 实际运行验证

### 包内依赖与认证存储

直接从最终发布目录解析并导入 worker，主程序被 suppress，不绑定端口、不启动浏览器。
`playwright-core 1.59.1`、`ws 8.21.0`、`selfsigned 5.5.0`、`@aws-sdk/client-s3 3.1033.0`
均精确匹配包内依赖声明，解析位置在 payload 内，没有回退到开发目录。

另从包内 checkpoint 模块完成合成 local SQLite roundtrip：新进程恢复已提交状态、旧版本
CAS 冲突拒绝、数据库中无合成敏感值明文、失效 tombstone 生效、seed 未改。key 仅在测试
进程环境中生成并传递，不写回执。未启动浏览器或调用 Google。Node 22 的 SQLite
experimental warning 保留；这不是生产 S3 或真实认证旋转验收。

### 原生桌面与进程生命周期

端口 4359、全新独立后端/UI 数据目录、非 payload 工作目录下启动最终包：

- `/healthz`、`/readyz`、管理页面均 200，worker 路径解析在新包内。
- 原生 UI 存活、响应且有 window handle；观测到 7 个本轮数据目录所属 WebView2 进程。
- 收尾仅停止本次创建的精确后端/UI 进程；它们和 WebView2 均退出，4359 已释放。
- 受保护端口的 listener 快照前后一致；未重启或替换现有 Gateway。

这证明原生进程能运行，不是人工视觉审查或全部桌面操作回归。

### 新 EXE 公共 API 合同

通过真实 loopback TCP 请求最终 `gateway.exe`，端口 4360，只有合成 provider/credential。
最终 **12/12** 用例通过：

- 六条请求的未鉴权分支均 401，零上游请求。
- Web 图片 `n=2`、Required 策略下的音乐/视频返回预期 400 和错误码，零上游请求。
- Canvas 音乐/视频均明确返回 503 / `gemini_canvas_host_media_contract_unavailable`，
  没有拿普通 Web、官方 Key、另一个账号或浏览器重新生成作为成功。
- `/v1/audio/speech` 鉴权后返回 200、`audio/wav`，与合成的 48 字节 PCM WAV 完全一致；
  引导 GET 仅一次、`XqA3Ic` POST 仅一次，POST 携带精确原文。
- 测试进程退出且 4360 已释放。WAV 只是合成容器，不是可懂语音或 Google 生成成功。

前两轮失败均保留：第一轮夹具把路由写到被原生 local startup 覆盖的路径，第二轮夹具
缺少媒体 owner 的 runtime-state locator。按真实启动 owner 改为合成数据下
`local/routes.yaml` 并补齐合成 locator 后通过；没有为夹具改变 Gateway 产品行为。

## 验证记录与边界

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r15/`。
总回执 `validation.json` 汇总来源、完整包 hash、build/native/worker/checkpoint/API 回执及
证据 hash。API 最终取 `packaged-api-acceptance-r3.json`，前两份失败回执不算通过。

本轮重新通过 checker 31/31、effective-line ratchet（3702 文件、14 个分类 immutable
runtime assets、受治理源码超过 700 行为 0）、`git diff --check`、新增文本 UTF-8 无 BOM
和外部夹具语法检查。没有改 policy、baseline 或 exception；没有重跑全量 Rust/Node 回归。

收尾在线 4200 的 health/readiness 均 200；仍为 r10，UI PID 36304、Gateway PID 46468。
没有修改在线账号、凭据、配置和数据库，没有提交、推送或部署新包。新包默认也使用 4200；
用户双击时若旧实例仍占用端口，会明确拒绝抢占，而不是停止旧实例。

仍未关闭的内容：

1. **实现/上游合同阻塞**：Canvas 音乐和视频尚无可信可用宿主传输。现有失败证据不能靠
   改模型名、移除准入保护或把普通 Web 成品标成 Canvas 来解决。
2. **真实业务验收未完成**：普通 Web 的图片恢复、媒体输入/输出、原文朗读，以及 Web/Canvas
   专用转写和 MP3 输入，尚未在本轮新包上取得真实账号公共 API 成功回执。
3. **存储环境验收未完成**：真实 Google 认证旋转/跨重启恢复、生产 S3 条件写回仍未验收。
   checkpoint 默认关闭，HTTP 路径没有自动写回，CDP/persistent profile 仍保留原生 owner。
4. **人工验收未完成**：新包桌面交互和用户场景回归。

没有重试此前被工具拒绝的真实 storageState 导出/隔离 worker 组合，没有换入口绕过，
没有重放已消费的媒体生成 guard。新包可交手测，但不能宣称 Gemini 全部多模态已完成。
