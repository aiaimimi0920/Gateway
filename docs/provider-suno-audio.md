# Suno Web 完成音乐的资产准入

本合同只描述 Gateway 无凭据开发的 MP3 完成态准入，不是当前 Suno wire、真实歌曲生成、授权/额度或完整渠道验收。

## 公共行为

正常 `/v1/music/generations` 保持 JSON `music.generation` 和原 `data=clips`，不增加 base64、不改成 Binary/SSE、不改写或代存资产 URL。direct、本地 browser 与 remote browser 共用最终 Music owner；只有 `completed=true` 时才读取并验证**全部**原 `audio_url`，任何一首读取/解码失败，整次调用失败，不返回部分成功。不将生成的 URL、扩展名或 `Content-Type` 当有效音频。

显式不等待且尚未完成时仍返回 `completed=false` 与原 pending 元数据，不读取尚不存在的音频；该响应不承诺音频有效。已完成结果即使请求不等待，也必须经过同一准入。原生成/轮询/最终处理的总 deadline、调用方取消和禁止 retry/fallback 保持；失败不自动重新生成、换账号、执行付费 download 或 WAV 转换。停止本地等待不代表撤销已经被远端接受的生成。

## 传输和资源边界

- 第一次资产 GET 前检查整个完成集合：非空、最多16个、ID非空/唯一/有界、状态完成、URL合法；remote worker 也不是可信输入。此检查不补造远端缺失的提交ID关联证明。
- 复用匿名公网资产 owner：HTTPS、无 userinfo/fragment/显式非标准port；DNS最多16地址且全部公网，绑定已检查地址，TLS验证原hostname；禁proxy、禁redirect、只接受HTTP200。不带生成Cookie、Clerk bearer、caller Cookie、Origin、设备或browser-token。
- 每资产最多32MiB、本请求累计64MiB，按剩余预算逐chunk读取；压缩HTTP实体由共享client解压后有界累计。传输栈可能先产生一个chunk，该上限不是wire流量或整个进程heap的严格上限。
- 全局最多2个MP3资产读取/解码owner，在读取前准入；每次只保留当前资产及一帧PCM，不跨歌曲保留字节。单首解码samples上限20分钟；排队/下载/逐帧解码共享原期限。
- blocking decoder持有原permit；caller Drop/timeout设置协作取消标记，每帧检查。不能抢占已经开始的单帧运算；该帧结束后退出并释放permit，不后台续跑整个文件或生成任务。

## 支持范围与保证边界

仅准入 MIME `audio/mpeg`、`audio/mp3` 或 `application/octet-stream` 的 MPEG-1/2/2.5 Layer III（MP3）。按真实header逐帧精确消耗并交固定 Symphonia `0.5.5` 完整PCM解码；检查非空、有限samples和一致signal。拒绝free-format、其他容器/codec、坏header、帧内截断、帧间垃圾和尾部HTML，不通过搜索下一个sync恢复“成功”。

允许一个有界ID3v2前缀及ID3v1尾标签；验证标签边界/版本/flags/长度和v2.4 footer，不解析metadata内部语义。未知容器、APEv2等标签不自动剥离后接受。

固定库会把 reservoir underflow 对应的缺失granule补成静音并继续返回PCM，因此 `decode Ok` 不能独立作成功证明。Gateway在每帧解码前读取固定side-info中的 `main_data_begin` 与各granule/channel的 `part2_3_length`，跟踪实际未消费字节，拒绝首帧或任意后续帧引用不存在的历史，也拒绝声明消耗超过可用数据。没有修改第三方decoder或拦截全局日志来猜测失败。

MP3无统一文件结束标志：刚好缺少若干**完整尾帧**、或以独立解码帧边界构成的较短合法文件，不能只凭当前body检测为被截断。未把原metadata duration当可靠精确时长，不做听感/内容/版权判断，也不声称验证MP3 CRC（当前依赖读取但不校验CRC）。资产URL在响应后仍可能过期或变化；本合同证明本次实际读取字节的准入，不保证未来再次读取相同内容。

## 错误、账务与接力

资产错误使用Suno安全错误码和502；原期限用尽为504。省略signed URL、CDN正文及decoder/连接细节。CDN401/403不直接当生成账号认证失效；所有此类后提交失败都禁止重放。

Local 次数 Key 由公共请求 owner 统一扣款、退款与审计。上游 usage／credits 未知时不填 0；音频准入也不补足现金 Key 所需的用量或费率证据。精确媒体探测、逐次费用确认和管理界面已有受控联合验收；适用范围和仍待确认的真实平台、Server 与打包结果见[渠道合同](provider-suno-channel-contract.md)。

库许可与固定源码见 [Symphonia 说明](third-party/symphonia/NOTICE.md)。仍须验证许可随适用包交付；源码说明不能替代打包门禁。
