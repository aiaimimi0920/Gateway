# SenseNova：包内账号模型目录回执

本轮验证真实管理路由到隔离合成上游的链路，不使用真实 Key、订阅或付费调用。

- 桌面 `devtest`：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-sensenova-catalog-20261122-231803`。暂存及发布目录校验均通过，包校验索引覆盖 6170 个载荷；后端 SHA-256 为 `12ea298034cc223ae565debeedff9f3bc288427165d22f4311383f83a47dc3bf`，桌面为 `8e553d9fc30c9aa2260a1e1f52e3a1bc1d04bba01504e5901c427540d9006ba9`。
- `gateway.exe` 使用环回 Server、专用 PostgreSQL／Redis 和不转发的 TLS peer。启动缺 CA 被拒；管理 API 缺身份返回 401、缺敏感授权返回 403、非合成 Key 返回 400。草稿 Key 和已保存账号各读取成功一次，确认去重、排除图片／视频输出、绑定活动 revision、不写配置、不回显密钥、零推理及正常 drain。
- 回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-catalog-f478c11b740048b5add7cf017067f389/result.json`。后端与 manifest 哈希匹配，`catalogCalls=2`，`inferenceCalls=0`，`routeRevisionUnchanged=true`。可复跑入口见 [包级夹具说明](../../tools/packaged-provider-fixture/README.md#sensenova-账号目录)。
- Rust 编译检查、SenseNova 定向 **39 通过／0 失败／5 个 opt-in 测试未随此命令运行**；桌面 typecheck、目录与表单用例 **19/19**；Python 夹具 **6/6** 连续五轮、56 项功能线清单、相关 Rust 格式、有效行 checker **31/31**、ratchet 与 `git diff --check` 均通过。Windows 的拒绝测试使用不带正文的请求验证提前拒绝，避免未读取的负对照正文使 TCP 关闭抢在 HTTP 响应前。

Chat 视频 wire、控制台 OAuth／积分池的公开合同及资格、真实 Key／订阅／费用均不在本回执内。

## 2026-11-23：原生账号目录保存及重启

复用同一经过哈希校验的 devtest。原生 WebView2 以 `existing` 模式连接隔离 Server，使用合成管理身份，在已保存的 SenseNova 账号编辑窗实际点击「拉取模型」。请求仅提交 `credentialId`；表单展示两条去重后的文本模型，再通过「保存」触发正常 `PUT` 并取得新 revision，没有用脚本代替 UI 提交。退出后重建后端及原生窗口，管理读回确认目录和已配置的凭据，并在编辑窗读回相同目录、API Key 不回填。独立回执中目录恰好读取一次、零推理，两个窗口已退出、调试端口已释放、后端正常 drain；专用容器已停止，隔离数据保留。

- 原生通过回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-native-62df4935157e403784ef089452f1741c/result.json`。
- 此证据验证隔离 devtest 的已保存合成账号工作流；不证明官方模型可用性、真实账号授权或任一付费行为。
