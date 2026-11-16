# Suno Web：实际 MP3 读取、解码与公共音乐结算

日期：2026-11-08。R15 / W11 的限定完成音乐子链；不是整卡 `offline_ready`、P50/P60/P70 或总目标完成。

## 本批改了哪些内容

在原三适配器/公共音乐调用的共同 Music finalize 中接入原始 `audio_url` 准入，覆盖 direct、本地 browser 和 remote browser。完成集合先整体检查，再逐首匿名读取并完整解码；任何一首失败都不能返回部分成功。原JSON/clip字段/URL保持，不增加base64或二进制端点。显式pending不读取未完成音频，不声称它已有效；完成结果即使请求no-wait也不能绕过准入。

复用现有公网资产transport，不携带Clerk/Cookie/caller身份、禁proxy/redirect、DNS绑定；32MiB/首、64MiB/请求、2个全局owner、20分钟/首的解码samples上限和原总deadline共同约束。只读取原播放URL，不调用billing download/WAV转换，不重复生成或换号；失败省略signed URL、正文和内部连接/解码错误，CDN403不是生成凭据过期证明。

固定可选依赖Symphonia `0.5.5`，仅开启MP3，新增5个lock package，没有升级/移除旧package；不依赖用户安装FFmpeg。逐帧精确消耗、ID3边界和完整PCM解码之外，交叉审查发现该库会把reservoir underflow补成静音后返回Ok，已增加每帧side-info历史引用/真实可用字节检查。新增测试独立断言依赖仍容错，但Gateway必须拒绝首帧或后续帧缺失历史。库源码未修改，MPL许可及固定源码获取说明保留；P50仍需证明随包交付。

公共验证复用正常 `/v1/music/generations`、正常管理Key和真实Local SQLite：精确原账号/模型归属、成功次数扣一次、第二首资产失败整次退款、读取中caller Drop的cancelled审计/退款/permit归还及健康中性；上游usage未知仍未知，不为通过测试伪造token、credits或现金结算。明确有可用backup仍不得fallback/replay。

实现和精确保证边界见[音频合同](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/provider-suno-audio.md)。

## 运行证据与过程失败

证据目录：[本批运行产物](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-suno-audio-owner-20261108)。临时脚本、SQLite、日志与原始公开资料均放在该目录，不操作真实凭据或既有实例。

- `red-audio`：1项真实红测失败；旧实现把audio/mpeg声明下的HTML当作完成音乐，未读资产。
- `green-first`：编译E0603，新增函数可见范围过窄；只修本模块可见范围，不放宽公共API。
- `green-complete`：151 passed / 3 failed；三项公共测试均停在夹具将路由模型 `chirp-v3-5` 错当wire模型的断言。本仓既有映射输出 `chirp-auk-turbo`，修夹具为既有常量，不改生产映射或声称它是当前平台模型。
- `mp3-first`：当时9项解码测试通过；早于reservoir修复，不充当最终证据。
- `green-final` / `green-final-r2`：分别为新增remote测试放错私有字段可访问模块的E0616、移动后遗漏显式path的E0583。只修测试归属，不将生产字段公开。
- `green-final-r3`：155 passed / 0 failed，但期间并行River接入修改了21项选定输入，前后来源检查失败；不当作最终工作区已验证，也不覆盖对方修改。
- `green-final-r4`：155 passed / 0 failed，期间又有31项Cloudflare/Magpie选定输入变化；同样保留为中间绿测，不冒称当前来源通过。
- `adjacent-final`：70 passed / 0 failed / 0 ignored，含最终11项MP3测试；排队期间三个其他任务的Rust文件变化，仍不独立充当最终来源证明。
- 范围审计首次因上游MPL原文的尾空白失败；只修审计为逐字节核对固定原文，不改许可内容或生产实现。后续又准确拒绝独立Magpie integration test变化；已按真实lib dep-info和Cargo `--lib`边界单列不消费该文件的检查，而不是重复编译无关库或笼统忽略源码漂移。

最终 `contract-final` 在同一个当前lib harness统一执行Suno、11项MP3及相关headers/ChatAIBot/Udio/Luma/公共lifecycle：**224 passed / 0 failed / 0 ignored**，含20项本批新增测试；首次测试的154项完整身份全部保留。中间155/70不再相加，前后4492项选定输入无漂移。

固定快照下，正常 `runtime-defaults` 及仅移除Suno leaf的两种生产lib编译通过；15份Rustfmt、checker31/31、ratchet（5024文件、14个immutable artifacts、governed source >700=0）、`git diff --check`通过。15份Rust最大257有效行，新模块/测试最大214；baseline/policy/exceptions未改。27份范围文件、25份UTF-8无BOM文本、合成夹具及四份HTTP200固定原文/许可的hash与直接写前diff均有审计。

后续最初只有另一任务的独立 `tests/console_magpie_api_catalog_contract.rs` 变化；该文件不在真实lib测试dep-info中，不以此重跑无关编译。静态收尾又出现Grok OAuth开发的device/mod/registry/session及独立前端测试变化，审计再次如实拒绝“全部当前来源一致”。这些后续共享输入**没有联合重新编译/测试**，不能把224通过与两生产lib编译冒称最新全工作区已绿；本批Suno范围的构建输入hash仍与已通过快照相同。最新ratchet/diff另留收尾回执，不能补造后续共享源码的业务证据。

`audit.json`固定引用 `contract-final-after.source.json` 的4492项已验证输入，同时单列收尾4496项快照/逐阶段后续差异及当前范围内零漂移。`external-paths.json`保留47项已核并行路径（含共享Cargo.toml/tests.rs重叠改动）；随后另有8项Grok相关外部路径仅登记在审计、未审查或修改，不把它们计入本批通过范围。后续ratchet记录5028文件、exit0但期间源码继续变化，diff也exit0；这些收尾结果不是原子冻结或最新共享工作区绿测的证明。River/Cloudflare及后续Grok改动不是本批实现，不覆盖、不停止其任务，也不无限追赶变化重编译。本回执的通过结论以固定快照及本批未变范围为限，而非当前整个共享工作区。

有效行生成报告没有本批直接写前镜像，且有并行checker写入；审计只保留当前报告到证据目录，不用前批或Git HEAD覆盖工作区报告。未将选择性输入快照夸大为完整仓库/构建环境证明；本批未跑前端、Node/CDP、完整feature矩阵或整包，既有浏览器子批的未变证据不冒称本轮重跑。

## 剩余边界

本次仅准入MP3，不增加WAV/Ogg/AAC等未实现格式；不验证MP3 CRC或听感/内容/版权。MP3没有统一EOF标记，完整尾帧缺失可能仍构成较短合法文件；不把metadata duration当可靠长度证明。原URL未来可能过期或变化，不保证响应后字节永久相同。remote结果只保留既有提交归属合同，本次不补造接口未提供的提交ID证明。

正常公共router→合成上游TCP→Local结算不是独立下游TCP、真实UI、Server/PG、跨进程、EXE/整包或真实Suno生成。当前模型/feed wire、Clerk/device/challenge/refresh、精确媒体probe/逐次费用/UI、真实CDN/权益与完整媒体参数仍缺；OAuth/MCP依旧discovery-only。Images旧下载和Video仅URL分支未在本批改写，不借MP3证据宣称它们已安全/有效。

本计划模板累计仍17（不计并行且不属本计划的River/Cloudflare），无新增整卡 `offline_ready`。只退役“限定完成音乐实际MP3准入与正常公共Local结算”缺口，不将待补边界整体消除。未使用真实凭据/生成/付费下载、未创建Redis/Docker、未出包/提交/推送，已消耗的P34容器授权不复用。总目标继续active，不标complete。
