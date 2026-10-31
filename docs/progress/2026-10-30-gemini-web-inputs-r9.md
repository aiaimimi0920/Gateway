# Gemini 普通 Web 多模态输入与专用转写（r9）

## 本批解决的缺实现

此前普通 Web 的文字 replay 明确拒绝图片、音频和原生 Gemini inline 媒体；公共
`/v1/audio/transcriptions` 也只有 Canvas browser adapter 接入。本批补普通 Web 自己的
上传与 StreamGenerate 路径，不使用 Canvas host、官方 API Key 或额外浏览器作为回退。
普通 Web 与 Canvas 仍是独立入口，没有改写既有账号、凭据、数据库或存储版本。

2026-10-30 联网读取公开参考
[HanaokaYuzu/Gemini-API](https://github.com/HanaokaYuzu/Gemini-API/tree/1ad0d4a93e505e77c79769d78ef2997641f973c8)，
固定 commit 为 `1ad0d4a93e505e77c79769d78ef2997641f973c8`。
仅参考 `utils/upload_file.py`、`constants.py`、`client.py` 中的上传合同、文件引用结构和完成指标，
独立实现有界 Rust owner；没有引入 Python 运行时、复制依赖或采用其自动重试机制。
公开参考不是本账号当前上游成功证据，也不推定它的全部协议字段必须替换已有可用文字合同。

## 改了哪些内容

- OpenAI data URL 图片、`input_audio` 的 MP3/WAV 和原生 Gemini inline 图片/音频/视频，
  通过独立准入 owner 转为上传 bytes。最多 16 个文件、合计 16 MiB base64；不读调用者文件名、
  磁盘路径或远程 URL，不隐式重编码。仅用户媒体可上传；未知字段、重复 alias、无法表达的
  `videoMetadata`、非 auto 图片 detail、非法或空 base64 在任何网络动作前拒绝。
- 上传固定到 `https://content-push.googleapis.com/upload`，使用当次 app bootstrap 的 Push-ID
  和 `bard-storage` tenant；不跨域复制 Cookie、Authorization、模型或调用者 header。
  文件名按 MIME 内部生成，不信任原上传文件名；禁止 redirect，不接受上游提供的任意上传地址。
- 串行上传共用截止时间，最多额外持有一个 multipart body；响应体上限 4096 bytes，
  只接受受限 `/contrib_service/` 文件引用。错误不包含上传响应正文或媒体 bytes。
- StreamGenerate 只接收文件引用，不将内联 base64 塞进 prompt。普通文字仍走原解析器；
  新媒体输入要求原生 Web candidate 的 `[8][0] == 2`，或显式 `STOP` 的非 thought 文本。
  部分文本、thought-only、工具调用和超过 1 MiB 的候选不冒充完整媒体答案。
- 新媒体的下游 SSE 复用既有序列化器，仍是先聚合再输出，不声称逐 token 流式。
  上游正文改用既有 bounded collector；新媒体任一步错误均不可自动重放或换账号。
- 普通 Web `/v1/audio/transcriptions` 增加独立 selector 和 JSON/text dispatch，上传原音频
  bytes 后提示式转写。支持 MP3/WAV/FLAC/Ogg/WebM/M4A 音频 MIME；仅缺少 MIME 时用扩展名推断。
  合法的无 filename multipart 按明确 MIME 处理。支持两位小写 language 和有界参考 prompt。
  不伪造时间戳、duration 或 segments；srt/vtt/verbose_json、temperature 控制、stream、
  timestamp/include/chunking 等未实现参数明确拒绝。它不是强制对齐 ASR。
- 交叉复核修正无 filename 被 null 标量校验误拒绝，以及为隐私清空正文后丢失 HTTP 200
  challenge/session-invalid 分类的问题。分类使用已观察的布尔证据，错误投影不保留私有正文。
- 公共 multipart 红测发现默认 UI 模板 `gemini-web-chat` 使用 `gemini_web_compatible`，
  而新增转写分派最初只覆盖 modular adapter，导致实际入口落入通用 planner 并返回
  `unsupported_gemini_web_request_plan`。JSON/text 分派现同时覆盖两个既有 adapter；
  公共负例与 selector 合同均覆盖默认和 modular preset，不改模板或已有账号记录。

格式/MIME/base64 准入不是完整媒体解码，也不能保证每个账号或模型均有媒体理解权益。
不把普通 Web 的新音频链提升成 Canvas 音乐、视频支持。

## 检查和交付证据

新增普通 Web 转写 selector 回归在旧实现上先失败：候选仍被拒绝，随后才补执行链与放行。
第一轮相关 Rust 单测 45/45 通过，包含真实 loopback HTTP 的 bootstrap → upload → generate
次序、原始 bytes、文件引用、跨域 header 排除、坏引用/过大响应/上传失败不生成、无效输入零网络，
以及旧文字解析和 SSE 兼容。加入交叉复核回归后，最终 Web 单测 47/47 通过，无 skip。

首次公共 ingress 检查为 Canvas 6/6 通过、普通 Web 1/1 失败，暴露上述真实模板 adapter
接线遗漏。修复后两个测试目标共 7/7 通过，无 skip；普通 Web 单项内部覆盖两个 preset
各四个鉴权/媒体/格式负例。绿测为 `rust-ingress-alias-fixed.log` 及 JSON 回执，失败日志保留。

新增公共 multipart 检查使用真正的 local runtime、路由、鉴权、JSON/text dispatch 和合成账号
隔离数据；上游只绑定随机 loopback 端口。它证明错误到达 Web owner 且在网络前拒绝，
不是成功生成、真实模型识别或公共媒体成功响应的替代证据。

本批所有测试只用合成身份、内存媒体和 loopback；未执行前轮被策略拒绝的会话导出/新 worker
组合，也没有换入口或凭据规避该拒绝。测试临时目录显式设在本批 linshi 下。

effective-line checker 31/31 及 ratchet、cargo fmt check、diff check 通过；19 个修改文本
严格 UTF-8 无 BOM。分派 owner 为 379 行，公共 Web 合同 137 行，Canvas ingress 根合同
142 行；未改 baseline 或例外。Rust 编译仍有既有 unused/dead-code 与 linker warnings，
不是零告警构建。未重复全量回归或 strict 迁移审计。

最终定向检查、源码来源、手测 EXE 哈希及隔离原生 smoke 记录在：
`linshi/gateway-gemini-dual-mode-20261030-r1/r9/delivery.json`。
目标手测包为 `gateway-devtest-20261030-gemini-dual-mode-r9`；仅在该回执实际通过后可称为交付。
不覆盖 r8，不创建便携 `.ng`，不自动提交、推送、替换或重启运行中的 4200。

## 总目标仍需的证据与实现

- 新普通 Web 多模态输入/专用转写、既有普通 Web 图片/编辑/TTS/音乐/视频的真实公共 API
  成功验收，以及 Canvas 专用转写/MP3 识别仍未完成；离线 fixture 不计作业务成功。
- Canvas 音乐、视频仍无已确认可用宿主合同，继续明确 unavailable，不能退回普通 Web 或 API Key。
- JSON 导入 seed 跨重启 checkpoint、多 worker CAS、S3 回写仍未实现；r8 的源 freshness
  与本批 Web HTTP 上传都不能代替长期会话持久化。
- 既有 modular Web 图片生成路径在 direct HTTP 失败后仍可能转 browser 再发生成；本批
  上传/识别的禁止重放保护不覆盖该旧输出路径，尚未修复，也没有观察到真实重复生成或双扣。
- 本批不声称所有 Gemini 多模态均已完成；未运行付费上游回归、正式发布审计或 Docker 构建。
