# Gemini 图片只读取回 Cookie 链修复与 r18 手测包

## 当前结论

关闭一项实际图片取回缺陷：生成已被接受后，只读 bootstrap/history 原来复用静态 Cookie，
丢弃成功响应的 Set-Cookie；上游在读取中轮换或删除 Cookie 时，后续读取可能失败。
现在同一取回过程使用请求局部 jar，bootstrap 和 pending history 更新都能传给后续读取。

**本轮不是 Gemini 全部多模态完成。** Canvas 音乐/视频仍有下述核心 blocker；没有用普通
Web、官方 API Key 或合成结果冒充 Canvas 成功，没有再次执行被拒绝的真实会话导出流程。

## 改了哪些内容

- `canvas_assets_part_1/image_recovery.rs` 新建请求局部 `SessionCookies`，不安装共享 jar，
  不改变 caller-owned seed，不持久写 Cookie、凭据或浏览器状态。
- `image_recovery/http.rs` 每次固定同源 bootstrap/history 请求前重新 apply，按目标路径筛选，
  包括清除已删除 seed；成功、限量、非 challenge/session-invalid 正文后才 receive。
  bootstrap 还须先解析必需的 token/build/session 字段。history 解析失败直接退出，不能再轮询。
- 复用原有 Cookie owner 的域、路径、Secure/Host 前缀和容量限制，不写另一套 jar。
  仅扩大到 `crate::upstream` 内可见，不新增对外公共 API。
- 保留单次生成、固定 turn/candidate 绑定、禁止 redirect、最多 6 次只读轮询和 30 秒共享
  deadline；取消直接 drop，没有 detached poller、子进程或额外阻塞文件 I/O。
- 新增四项回归：bootstrap 轮换、pending history 轮换、删除不复活、路径/域隔离。每项复用
  同一 HTTP client 执行两次取回，确认状态不会泄漏到下一请求，初始 seed 保持不变。

这不是生成响应 Cookie 捕获或 Web 图片跨请求/跨进程 HTTP checkpoint，未改 legacy
preflight，也未扩大 r17 默认/modular Web checkpoint 的覆盖范围。

## 实际验证

1. **旧 r17 EXE 公共入口红测**：合成模板/账号，`/v1/images/generations` 一次生成被接受，
   bootstrap 200 后，第一次 history 因未携带更新 Cookie 返回 401；Gateway 返回 503 /
   `gemini_web_image_read_http_failed`。恰好一次生成，没有失败重放。
2. **新 r18 EXE 公共入口绿测**：同一合同返回 HTTP 200 和预期合成图片 URL；严格计数为
   一次 StreamGenerate POST、一次 bootstrap GET、两次 history POST。bootstrap 与 pending
   history 两次轮换均生效，path=/app Cookie 未发给 history，源合成状态文件 hash 不变。
   这验证 URL 取回合同，不证明生成了真实 Google 图片或完成了媒体解码。
3. **Rust 54/54**：图片单次提交/取回 13，共享 Web owner 34，图片绑定/解析 7。新增四项
   回归先得到预期失败，再在独立 green 重跑中全部通过。既有编译 warning 保留，未 cargo fix。
4. **代码与静态检查**：checker 31/31，ratchet、formatter、diff check 通过；3718 个文件，
   14 个 immutable runtime assets，受治理源码超过 700 行为 0。六个相关 Rust 文件全部
   不超过 250 有效行且 UTF-8 无 BOM，没有修改 policy、baseline 或 exceptions。
5. **原生新包启动**：4367、foreign cwd、全新 UI/backend 数据；health/ready/UI HTML 200，
   worker 路径在包内，UI 有 window handle、响应正常，7 个 WebView2。仅停止本次进程，
   WebView2 全退出、4365/4367 释放，保护端口 listener 快照一致。未做人眼视觉或桌面业务验收。

有效行 before→after：mod 42→43，session_cookies 191→195，image_recovery 52→55，
http 135→152，既有 recovery tests 125→127；新 Cookie 测试 130。职责为同一取回会话链，
没有把新状态放进共享 client，容量和资源 lifetime 均沿原 owner。

首两次测试启动被取消，不计通过；保留了失败日志。旧源码 lib-test 编译实测 22m53s，
补丁后的 green 编译 12m57s，执行 13 项约 22.81 秒；本轮受依赖缓存和同机 Otter 编译影响，
这些不是速度基准。未重跑全量回归、付费上游或正式发布审计。

## 新 desktop devtest 包

目录：
`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-20261031-gemini-dual-mode-r18/`。

官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4` exit 0，构建打包实测
595.641 秒；来源 revision `fd74db9c374d8caa8309686592a0908c70376773`，包含构建时 dirty tree。
构建前后及打包来源指纹 `1c6ef3d6002c981d530b352e4513371d905b45fc1ebbb89ae4c0047425929d50`；
provenance builtAt `2026-10-30T20:28:21.5799729+00:00`。

清单 **5904 项逐项重算 bytes/SHA-256 通过**，完整目录 5906 文件、132823437 字节，无 `.ng`
或 `scripts/tests`。官方 artifact smoke、锁定 worker 依赖导入和上述新 EXE/API/native 检查通过。

| EXE | 字节数 | SHA-256 |
| --- | ---: | --- |
| gateway.exe | 82676224 | `1f74d2916cdf3096260f10d76f635bd6e90127b806bb5d0e76a264bfd30e9a31` |
| gateway-ui.exe | 12862464 | `0796d8d95ae97fc63f0bf36953bc15157492021e705e7de10be27b2298ad986a` |

本进度文档是打包后新增，不在 immutable 包内；不声称当前整体工作树指纹仍等于构建快照。
没有在包内补丁或写测试数据，没有提交、推送或部署。默认 4200，占用时拒绝抢占。

## 最后仍未完成的核心内容

- **Canvas 音乐**：当前宿主注入元数据没有登记允许的音乐模型/传输合同。不能猜一个
  `musicModelName` 字段、放宽 native fetch 或用官方 Key 接管来宣称完成。
- **Canvas 视频**：已观察到 `videoModelName` 及 predict/predictLongRunning 准入，但既有
  独立真实调用得到宿主内部 `[13]` / 空正文 401，未取得视频或 operation。成功响应、轮询和
  下载合同仍未知，生产 host 视频执行器也未接入；不是简单声称平台永久没有视频能力。
- **其他实现/验收**：Web 图片生成响应 Cookie 捕获及跨进程 HTTP checkpoint 未接入；真实
  Google 多模态/认证轮换、生产 S3 条件写回和人工桌面业务回归仍未验收。HTTP S3 driver
  本身仍未实现，不应把可选存储扩展当作 Canvas 多模态完成的替代。

本轮重新联网取得 Google [Canvas 帮助](https://support.google.com/gemini/answer/16047321?hl=en)
和 [Lyria GenerateContent 文档](https://ai.google.dev/gemini-api/docs/generate-content/music-generation)
正文；官方 API 能力不证明当前账号的 Canvas 宿主允许相同模型/操作。没有真实凭据调用，
没有重放已消费的媒体 guard，也没有通过其他入口绕过已有拒绝边界。

在线仍保留 r10 的 UI PID 36304、后端 PID 46468；收尾 healthz/readyz 均 200。证据根：
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r18/`，
以 `validation.json` 的实际测试、哈希、进程/端口清理和未完成清单为准。
