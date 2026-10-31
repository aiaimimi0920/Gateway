# Gemini Web 已接受图片的只读结果恢复（r13）

## 原因与改了哪些内容

r10 禁止了不确定结果后的重新生成，但图片 StreamGenerate 仅返回任务标识、没有最终图片时，
单次提交 owner 仍然直接失败。本轮只补这个缺口，不恢复旧的 prompt follow-up、JSON 再生成、
模板重发、最新作品搜索或浏览器再次发送。

- 单次 Web 图片生成/编辑保留本次实际发送所用的会话快照。模板路径在发送前捕获，heavy
  路径在会话准备完成后捕获；恢复不从可变 storageState 重新挑选会话，不换账号、不写凭据。
- 已有最终图片的响应仍沿原来的资产处理路径直接返回，不额外读取历史。没有资产时，必须从
  完整结构化 StreamGenerate envelope 取得同一个 `cid/rid/rcid`。不从原始文字扫描 ID，
  不拼接不同 frame 的部分 ID，不接受冲突 metadata 或多个候选。
- 使用该快照向同源固定 `/app` 读一次新引导数据，然后只向固定 batchexecute 入口发送
  `hNvQHb`（LIST_CONVERSATION_TURNS）。参数仅为绑定的会话和固定 limit；没有原始 prompt、
  模型选择、`MaZiqc` latest-search、`MUAZcd` unread write 或 StreamGenerate 重发。
- 历史结果必须精确匹配本次回复和候选。只接纳 `completion_status=2` 的生成图片字段；旧
  turn、其他候选、pending、文本内 URL、重复/错误 envelope 均不能冒充已恢复的图片。
- 最多读 6 次，间隔 2 秒；恢复阶段总时限为调用方 timeout 与 30 秒的较小值，包含引导页、
  网络读取和 sleep。所有等待归属调用 future；取消/超时不遗留独立 poller。
- 新增引导/历史读取禁止重定向；声明长度和实际流累计均限制为 8 MiB。解析要求完整 framing，
  支持 UTF-16 长度，限制 envelope 数、历史 turn、候选和资产数量。恢复的图片 URL 仅允许
  不带用户名、密码、fragment 或非默认端口的 HTTPS googleusercontent 域，不接受任意下载目标。
- 失败不重生成、不刷新后重发、不切浏览器生成、不跨账号 fallback；新错误均不可重试且
  `Abort`，不复制私有上游正文、Cookie、token、会话 ID 或历史内容到错误消息/诊断文件。

接入范围是默认 Web 经 r11 的 request-local bridge，以及显式 modular Web 的图片 HTTP
owner。没有把这条历史读取恢复接到 Canvas host/旧 Canvas 的多发送器恢复链。

## 协议依据

使用已联网读取并保留的固定参考版本，不宣称它是最新版：

- [HanaokaYuzu/Gemini-API, chat_mixin.py](https://github.com/HanaokaYuzu/Gemini-API/blob/1ad0d4a93e505e77c79769d78ef2997641f973c8/src/gemini_webapi/components/chat_mixin.py)
- `read_chat` 的请求为 `[cid, limit, null, 1, [1], [4], null, 1]`；历史 turn 的回复 ID
  位于 `[0][1]`，候选列表位于 `[3][0]`，候选 ID 位于 `[0]`，完成状态位于 `[8][0]`。
- 本轮没有照搬参考项目挑选最新 turn、自动重试或重发生成的策略。

## 验证与交付

- 新增只读恢复测试 13/13 通过：7 个严格协议测试、6 个 loopback owner 测试。包含 generation
  POST 始终 1 次、pending 后取回本次 final、读取 RPC 参数精确且不携带 prompt、旧 turn/其他
  candidate 不成功、已有图片不多读、缺绑定零网络、6 次上限、总 deadline 和超时后无残留读取，
  以及重定向、会话引导无效、HTTP 错误、声明/流累计超限和私有错误遮蔽。
- 连同相邻模板/heavy 单次提交和旧恢复策略，第一组单测 26/26；provider retry 与协议路由
  单测 59/59；默认 Web 媒体、转写、原文朗读和 Canvas 公共入口合同 23/23。Rust 定向合计
  108/108，无 skip。均为合成账号/loopback，不是 Google 真实生成或图片下载验收。
- 新增 loopback 用例先红；补实现后修正了首轮接线编译中 `wreq::IntoUri` 和 `Abort.reason`
  的类型适配问题，最终编译和测试通过。初始失败及中间编译失败日志保留，最终结果取
  `rust-green-r2.log`、`rust-policy-routing.log`、`rust-ingress.log`，不把失败尝试算作通过。
- effective-line checker 31/31、ratchet、formatter、`git diff --check`、UTF-8 无 BOM 通过。
  没有修改依赖、baseline 或 exceptions；既有 unused/dead-code/linker 告警仍存在，不声称
  clippy clean 或全量回归通过。

既有文件有效行：协议出口 300→301、资产 owner 出口 363→365、单次图片 owner 50→54、
StreamGenerate owner 127→160、模板测试出口 136→138。新生产文件分别为协议绑定/解析 206、
framing 52、恢复 owner 52、只读 HTTP 135；新测试文件为 244、125、197。无新增超限文件。

证据根：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-gemini-dual-mode-20261030-r1/r13/`，
包含 `validation.json`、前后行数、原始日志和源码/证据哈希。

本轮不包含真实 Google 账号验收、新 EXE、部署、提交或推送。没有重试此前被工具拒绝的真实
storageState 导出/隔离 worker 组合，也没有换入口绕过或重放已消费的生成 guard。

## 明确的边界与剩余项

只支持含完整可信 `cid/rid/rcid` 的当前响应；缺 ID、被截断或结构不支持时会明确失败，而不是
拿旧图补结果。读取被限定在该会话最近 10 个 turn，不做无限翻页。已有图片下载/输出由原资产
owner 负责，本轮没有改写全部下载路径或证明真实 Google 图片已下载成功。

这些约束不是跨请求/跨重启 exactly-once。跨重启 checkpoint、多 worker CAS、S3 writeback、
Canvas 音乐/视频可信可用宿主合同，以及 Web 多模态、原文朗读、专用转写/MP3 的真实账号公共
API 验收仍未完成。在线 r10 包不含 r11、r12、r13 源码，不能拿旧包 readiness 当本轮业务验收。
