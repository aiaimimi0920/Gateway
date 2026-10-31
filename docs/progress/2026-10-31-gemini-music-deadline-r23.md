# Gemini Web 音乐取回截止时间修复 r23

## 范围与实际缺陷

继续 r22 的剩余模态任务，只修改普通 Web 音乐的下载 owner 和对应单测。Canvas 的音乐／视频
保护不变，没有修改账号、模型默认值、路由、凭据、数据库或在线进程。

真实页面的一次 MP3 download-event 超时仍没有已证实的根因，不能将下面的本地代码缺陷
冒认为该工具事件超时的原因。本轮没有重复点击历史作品的下载或生成按钮。

源码及红测证明两个独立的产品缺陷：

1. 旧 `Number(timeoutMs) || 30_000` 将剩余预算 `0` 重新解释成 30 秒，允许过期请求继续取回。
2. 菜单和纯音频点击各拿完整 timeout，统一计时器却只参与后续等待。菜单耗尽预算后，旧代码
   仍会点击纯音频；若 download 与 timeout 都已经 settled，`Promise.race` 还可能优先返回
   放在数组前面的 download，错误地报告过期成功。

两项新增测试先失败，旧套件 6 通过、2 失败，均为 `Missing expected rejection.`；不是编译错误
或夹具故障。补丁后 8/8 通过，相关回归亦通过。

## 改了哪些内容

- `scripts/gemini-canvas-browser-pool-web-music-download.mjs` 使用 `performance.now()` 的单调
  截止时间；显式零／负／NaN 预算在点击或注册 listener 前返回原有 504 timeout code。
  未指定预算仍默认 30 秒，正预算仍上限 30 秒。
- 菜单、纯音频点击、download-event、临时文件路径和有界读取共享同一预算。每一步执行前后
  检查剩余时间，慢菜单超时后不会继续点击纯音频，不延长预算、不补发生成。
- 保留只读取 Playwright-owned 文件、16 MiB 有界读取、最低 MP3 容器准入及原有
  listener / timer / download cancel-delete 所有权。这里没有新增任意 URL 或路径读取入口。
  共享截止时间约束 active work；没有声称底层已经开始的异步操作可以被 Promise.race 强制取消，
  或清理 API 获得了新的取消协议。媒体完整解码仍是独立真实验收项。
- 测试等待慢菜单在超时后结束，再确认没有迟到的纯音频点击或残留 listener。

生产文件有效行 39→52，测试 61→83。没有新增依赖，没有改锁文件，没有扩大内存／队列上限。
逐文件检查了输入、身份、秘密信息、下载文件所有权、异步失败和清理；最小补丁没有加入网络回退。

## 验证

- 定向红测：8 项中 6 通过、2 失败；修复后 8/8。
- 相关 Node 四文件 70/70，通过音乐下载、媒体结果轮询及单次提交保护；与上述 8 项有交集，不累加。
- 真实离线浏览器下载 1/1：隔离 context offline，所有页面请求本地 fulfill，合成 ID3 bytes
  经 production owner 取回，自己的临时文件删除，原有 listener 保留。不是 Google 音乐成功。
- 嵌套 worker 包内容合同 1/1；effective-line checker 31/31、ratchet、两文件 `node --check`、
  `cargo fmt --all -- --check` 和 `git diff --check` 均退出 0。
- 相对本轮 `before/` 的完整 scoped diff 已复核；源码、文档和回执经严格 UTF-8 无 BOM／尾随空白检查。
  测试 TEMP/TMP 位于本轮 linshi 子目录，没有运行真实账号模型请求。
- 本轮没有改 Rust 源码，不重复编译所有目标代替实际业务验收，没有生成或交付 EXE。

## 浏览器只读复核及边界

现有 Edge 能列出任务页面并读取标题；原 Canvas 控制句柄的 DOM 操作报 `Debugger unattached`。
按工具说明在同一 Edge 打开临时检查副本后，普通 Web 历史音乐页面可读，已有作品、下载控件和
59 秒播放器可见。pageAssets 没有发现可直接引用的 MP3／WAV 资产，仅有一个 video 资产；
没有下载封面视频、转码，或冒充拿到了纯音频。本轮没有再次点击下载、重做、分享或生成。

Canvas 检查副本显示原应用及预览标签，但外层预览正文为空。仅切换代码／预览，未编辑源码或
执行 Contract Check；没有取得恢复完整交互预览的证据，不能称为浏览器控制／预览问题全部修好。
两个新检查副本均已关闭，原 `Media Capability Check` 页面已 markDeliverable 保留且未刷新。
历史作品截图是 `music-existing-readonly.png`，不是本轮新生成媒体。

没有导出 Cookie / storageState、重试此前被策略拒绝的会话导出＋worker 组合操作，或用其他
入口绕过该拒绝。没有停止、替换、重启既有 Gateway，也未提交、推送或部署。

## 未完成的整体目标

- Canvas 音乐仍缺当前宿主明确允许的模型／传输合同。
- Canvas 视频仍缺成功的同步输出合同，或宿主允许的长任务轮询／文件下载合同。现有注册名、
  旧内部 `[13]` 或空正文 401 均不能当成成功，也不能据此断言整站登录失效。
- Web 音乐／视频和其余尚缺的多模态公共 Gateway API 真实验收没有被本地测试替代。
  r21 文档输入、专用转写／MP3 等本地合同成果保留，真实 Google 成功证据仍需单独补齐。
- 本轮源码没有打入在线 r20，不能把旧进程或历史 UI 成功当作新源码的上线验收。

回执目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-music-deadline-20261031-r23/`。
`verification.json` 记录最终哈希、计量、各 gate exit code 及上述未完成边界。
