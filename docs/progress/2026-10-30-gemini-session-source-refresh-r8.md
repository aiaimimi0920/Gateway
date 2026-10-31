# Gemini JSON 会话源更新与缓存失效（r8）

## 目标与定位

本批完成一个剩余闭环：本地 storageState JSON 或远端同 key JSON 对象更新后，
空闲浏览器 context 不得继续把旧导入内容当成当前源；源读取失败不得静默执行旧 context。
不修改原始凭据、不写回远端、不增加依赖，不做此前被策略拒绝的真实账号操作。

旧 context owner 只检查 key、连接与页面是否存活，不比较源内容；旧 resolver 在镜像存在时
不会重新读取 S3。因此只重建浏览器仍不足以解决远端同 key 更新问题。

## 改了哪些内容

- 新增有界 snapshot owner。JSON 的同一份 bytes 生成 SHA-256 并解析成 storageState；
  newContext 直接消费该对象，避免先 hash 路径、后由浏览器重读不同文件的竞态。
- 每次空闲准入检查本地 JSON 内容，不依赖 mtime 或大小。源内容变化时关闭旧 context、
  创建替代 context；内容不变时保留浏览器已旋转的 Cookie，不重新注入旧 JSON。
- 对远端相对 `.json` key 做 fresh conditional GET；304 只复用匹配 endpoint、bucket、
  region、path-style、本地定位与完整 key 的先前快照。无 ETag 时重新读取正文。
  同内容但新 ETag 只更新 metadata，不重建浏览器，也不反复下载同一版本正文。
- 远端 context 路径不使用 legacy 本地镜像作为认证权威，也不覆盖该镜像。
  请求级 S3 client 在成功、失败路径均销毁，不复用另一存储配置的凭据。
- 本地/远端源读取复用已有 16 MiB、30 秒截止和 AbortSignal owner；错误采用固定文案，
  不输出 JSON、SDK response body、Cookie 或 token。非法 UTF-8、JSON、根结构直接拒绝。
- 删除的 JSON 不会被自动建成空 profile；相对 JSON key 禁止路径越界。
- 生成中的 busy context 不强拆。refresh 自己持有 busy 状态，避免 idle/capacity eviction；
  同 key 的并发刷新合并，所有失败路径释放 busy/inflight。
- 保留 CDP 连接及原生 profile_dir 生命周期；不对 Chromium 运行中的数据库做内容 hash。
- 新模块加入嵌套包内容合同。

## 实际验证

最初 6 个定向用例中 5 个按预期失败，证实旧实现继续返回旧 context、按路径重读文件，
并且非法替换不会阻止旧 context 复用。修复后定向 Node 88/88 通过。

覆盖：同长度/mtime 替换、同源复用、不同 key 隔离、读后改写、busy 保护、8 路并发刷新、
非法/删除源阻止实际 dispatcher 操作、真实 S3 SDK 对接本地 loopback 的 200/304/404、
旧镜像保留、命名空间变化、无 ETag、同内容新 ETag、大小/超时/中断、CDP 与 profile 兼容。
另有暂停 refresh 读取期间的 idle/capacity 交错，验证不会误回收正在检查的 entry。

真实隔离浏览器检查 1/1，通过 about:blank 和 `example.invalid` 合成 Cookie，
没有访问 Gemini 或登录账号：源未变时保留 rotated 值，源更新后新 context 读取新 seed，
旧 browser 已断连，结束后 context/inflight registry 均为空。

嵌套 worker 包合同 1/1、effective-line checker 31/31、ratchet 通过；
最终静态检查与包状态以外部 `r8/delivery.json` 为准。测试产物通过 TEMP/TMP 指向 linshi。
MockTimers 有实验性 API warning，不称零告警。

## 出包前发现并修复的运行缺陷

对 immutable r7 包执行 `SUPPRESS_MAIN=1` 的独立 worker 导入，确定失败于 `ERR_MODULE_NOT_FOUND`：
缺少 `ws`。此前官方 verifier 只复制 `playwright-core`，未携带其余已锁定生产依赖。
没有改动 r7，也没有用开发目录 node_modules 补丁伪装修复旧包。

- 新增锁定生产依赖闭包 owner，沿 lockfile 的 Node 包定位规则收集 direct、nested、peer 与
  optional 依赖；只复制选中闭包，不带 dev/extraneous 或 `.bin`，不执行安装、升级或包脚本。
- 校验根依赖精确版本及 manifest/lock 一致性、已安装包的 name/version；拒绝链接、路径越界、
  必需依赖缺失和已存在目标文件。限制依赖图规模、目录深度及复制条目数。
- 包内 Gemini worker 以 main suppressed 实际导入，缺依赖或嵌套模块失败立即阻止发布；不启动
  worker、浏览器或端口，不读取真实账号材料。保留 ChatGPT runtime 导入及缩减 fixture 兼容。
- 新增红测最初 11 项中 10 项失败；最终 Node 16/16、相关官方 Python 包合同 12/12 通过。
  完整真实锁图已复制 132 个生产包，ChatGPT 和 Gemini 独立包内导入通过。
- 新增 Gemini pool 路径 owner：显式 payload root 优先，否则使用 EXE 相邻 scripts；只有 checkout
  target 下的开发 EXE 可回退源码。指定 payload 缺文件时不静默执行开发机脚本。
- 日志复用 ConsoleConfig 的 state owner、server routes 默认值和 payload 排除校验，落在
  `state_dir/logs`。不再写编译源码 `.runtime`，不更改子进程 cwd 或既有相对对象存储语义。
- Docker 的 EXE 位于 `/usr/local/bin` 而 scripts 位于 `/app`，不能沿 EXE 相邻规则猜测。
  镜像显式声明 `GATEWAY_RELEASE_PAYLOAD_ROOT=/app`，保留 `/data` 的 state/routes。
  新增布局合同先红后绿；没有运行 Docker 镜像构建或替换在线容器。
- 路径红测已证实旧行为忽略 payload/EXE，修复后 Rust 6/6 通过；包及隔离原生 smoke 以
  `r8/delivery.json` 为准，不把旧 r7 smoke 当作本轮运行证据。

代码量：旧 runtime helpers 489→485、启动接线 448→448；新增路径 owner 117、依赖闭包
owner 115、Node 合同 138、Python 合同 55 有效行。未新增例外，未重写 baseline。

首次 r8 打包期间补齐 Docker 布局合同，来源门禁以
`Gateway source tree changed during packaging` 正确拒绝发布。失败日志保留为外部
`r8/package.log`，最终重建/打包另记 `package-final.log`，不覆盖失败证据或旧 r7 包。

## 完成边界与未完成项

- 本批只完成 JSON 源 freshness 与 context 失效。persistent profile 原生状态仍归浏览器所有；
  原地替换一个正在使用的 profile 目录不在本批支持范围。
- 远端新路径仅覆盖相对 `.json` key；显式绝对路径继续指向本地源，非 JSON remote bundles
  保留原解析规则，不冒称所有远端 profile 格式都已支持。
- 一个已准入调用使用它取得的快照；源在该调用进行中变化不会强拆浏览器，下一次空闲准入再检查。
- 没有实现自动跨重启 checkpoint、多 worker 认证状态 CAS 或 S3 写回。
  原 JSON 仍是导入 seed，不是当前浏览器 Cookie 的自动回写目标。
- 真实 Canvas 转写、普通 Web 多模态公共 API 验收仍受既有策略限制；
  Canvas 音乐、视频依旧没有可信可用宿主合同，不能用普通 Web/API Key 回退冒充完成。
- r7 immutable 包保留；r8 必须使用新来源凭据打包，不能把旧包称为包含本次修复。
- 不重启/关闭在线 Gateway，不修改在线账号或数据库，不提交、推送代码。
