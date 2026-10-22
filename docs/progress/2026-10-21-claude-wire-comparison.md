# Claude CLI 与 Gateway 请求对照

## 授权与证据边界

用户指定 `https://ai.hybgzs.com/`、`claude-opus-5` 和一把 API key，用于隔离三路对照。
源码与报告不含密钥。证据根为
`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-claude-wire-20261021`，
含私有凭证与隔离实例数据，不能整体发布。日常 CLI/Gateway 配置不在修改范围。

本机实测 Claude Code 2.1.278。回环合成凭证 `baseline-01/02/03` 捕获非交互 SDK/原生默认
提示词模式，观察 Windows/x64、Accept application/json（stream=true）、
X-Claude-Code-Session-Id、实际 beta 列表和 `/v1/messages?beta=true`。

## 真实服务商前置验证

最小 Messages 请求 200，最小流式和同一 body 加完整 CLI headers 均 200。
早期自定义系统提示词的精简 SDK 模式在服务商返回 503：
`no available channels after filtering (group=claude model=claude-opus-5)`。
已中止自动重试；后续观察器在 Messages 首个错误响应头出现时结束自己的 CLI，不继续盲目重试。
单变量 probe 中添加该 SDK system 字段同样返回503；这只定位到该采样差异，不能推断服务商内部规则。

改用 CLI 原生默认提示词，`direct-native-02` 前三轮成功，第四轮429（上游负载饱和）。
稍后 `direct-native-resume-03` 复用同一隔离 HOME/session 完成第四、第五轮。
这是五个成功往返加一次失败/重试，不是无错误连续五次 HTTP 请求；原先失败记录完整保留。

修复前 Gateway 真正阻塞：`gateway-before-02` 返回400
`unexpected Anthropic message role: system`，没有发到真实上游。
因此不能把该次失败称为服务商拒绝 Gateway 指纹。

## 窄修

- 支持新版 messages 内 system，拒绝其中工具块；未变化的 canonical messages 可保留原块，
  不把 inline system 重复复制到顶层。模型映射、实际消息变更仍以最终计划为准。
- 受限原生身份/session/SDK headers 保留；普通SDK默认描述当前平台，不再固定MacOS/arm64。
- 单个 beta query保真；true/false不冒充beta feature。拒绝未知query、重复beta及配置冲突。
- Direct HTTP Messages流式Claude路径改为plain client，不继承Chrome默认头；禁止redirect。
- DSH会话别名转换；不注入CLI系统提示词、设备/账号identity或未具备的beta能力。

Gateway 抓包使用回环 HTTP reverse tap，tap 对指定真实域名使用验证证书的 TLS，
operator Host 配置明确指向该域名。这个切片验证 HTTP 字段和真实调用，不验证 Gateway
自身到上游的 TLS 握手。CLI直连切片使用仅进程信任的临时CA，不修改系统信任库。

## 真实抓包闭环

`gateway-after-01` 五轮全部 200，原始请求体字节、session 和凭证全部匹配，但发现请求头
大小写/顺序与参考不同，缺少 Connection。已按实测 2.1.278 的 HTTP/1 头序修复，
keep-alive 由实际出站连接池生成，不接受调用方任意 hop-by-hop 控制。

`gateway-final-01` 五轮全部 200 且包含 message_stop：原始 body 5/5 字节相等，
头名称大小写和顺序 5/5 一致，零缺失/新增头；排除每次对话不同的 session、长度、目标 Host
和认证位置后，静态头值也没有差异。会话值与入口一致，出站凭证与唯一授权凭证指纹一致。

`dsh-claude-01` 网页首轮成功，发现普通 SDK 输入的 Accept-Encoding 被沿用到上游。
新增窄修：仅普通 SDK 输入、且服务商没有显式覆盖时，按 Gateway 已启用的四种解码器
及参考格式发送 `gzip, deflate, br, zstd`；原生 CLI 显式值与 operator 配置保留。

`dsh-claude-final-01` 在真实 `http://127.0.0.1:3080/` 网页、默认工作区中新建会话完成五轮，
页面显示 5 轮 5 步、C1 至 C5；另有一次 DSH 标题生成请求，共六次上游 POST，全部 200
且有 message_stop。六次完整 body 均逐字节保留，出站凭证正确，共有头的大小写/顺序一致，
没有新增的 Gateway 专用头。截图：该证据目录的 `five-turn-ui.jpg`。

最后压缩修复后另跑 `gateway-final-02`：五轮全部 200，完整 message_stop；上述 CLI 字节、
头名称大小写/顺序、静态头值、session 和凭证检查再次全部通过。`final-verification.json`
记录闭环断言。两路导入的实际配置均与同一私有凭证匹配；HMAC salt 每进程随机，不跨运行
直接比较 HMAC 值。DSH 后 Cargo test 重新链接了 EXE，两次二进制 hash 不同，但生产源码
未再改变；分别保留 receipt 的真实 hash，不报告为同一二进制。重链接后的 local-06 也通过。

DSH 与原生 CLI 仍有以下有意保留的差异，不能报告为“完全不可区分”：

- 当前 DSH Anthropic SDK 的实际入口没有 session_id、session-id 或 x-claude-code-session-id，
  因而出口也没有 x-claude-code-session-id。不是漏转；已有别名转换通过回环验证，不伪造会话。
- anthropic-beta 根据实际请求能力配置，未复制 CLI 所有能力声明。其它共同静态头值一致。
- system、messages、tools、token 限制与标题生成是 DSH 的业务语义，不替换成 CLI 提示词。
  CLI 基线没有本次 DSH 的额外标题请求；Gateway 不增造这种调用来模拟调用频率。
- 原生 CLI 经本地入口会探测 HEAD /api/hello；本次真实直连上游捕获没有相应 HEAD，
  因此没有把本地探测改造成额外的服务商请求。

网页测试提供商为 `gateway-claude-wire-test`，只持有隔离本地 key；未把真实服务商 key
填入网页。原 Crow 会话已恢复，仍为 deepseek-v4.1-flash。测试会话和提供商保留作证据，
但隔离端口已停止，不是日常可用服务。

## 软件验证与范围

- 最终源码 `cargo build --locked --bin gateway` 成功。
- 最后压缩修复后的 `claude_wire_contract` 6 项、`request_client_profiles_contract` 8 项通过。
- 相邻回归：Anthropic 90、native_ 85；此前 headers 131、request_plan 30、profile 70 均通过。
  这些筛选套件可能有交集，不将数量相加冒充独立测试数；没有运行全库完整测试。
- `local-05` 最终二进制六组回环合成测试通过：原始 body/query/身份/session/头序，
  native 压缩值保留、普通 SDK 压缩规范化，凭证/Cookie/内部头边界，非法 query、错 key、
  session 冲突拒绝及 302 不向重定向目标重放 key。三次 fixture 请求，零真实服务商调用。
- checker 31 项及 effective-line ratchet、cargo fmt check、git diff check 通过。
- 主要有效行数：claude_wire 120、native_claude 145、Claude 合同 121、Messages 入口 88、
  normalize 391、request 342、profile headers 167。没有修改超过 700 行的责任文件。
- Cargo.toml / Cargo.lock 与本轮开始的 SHA256 不变；保留它们原有 dirty 状态，未改依赖。
- 2541 个 src/tests/docs 源码文档做授权 key 精确扫描，无命中。完整临时目录仍含私有凭证，
  不可整体发布。文本为 UTF-8，无 BOM。

保留失败记录：早期服务商 503、直连 429；旧 system 非法测试已更新为拒绝 developer；
一次最后合同测试因测试 Gateway 占用 EXE 而发生 Windows os error 5，停止隔离实例后
串行重跑成功。这些不是最终软件测试通过之前可忽略的成功样本。

不声称 Bun TLS/HTTP2 指纹完全等同，未验证非流式与跨协议的完整原生保真，不扩展 OAuth
辅助接口或 Cookie 会话合同。没有正式部署、提交或推送；以上是源码与隔离运行验收。
