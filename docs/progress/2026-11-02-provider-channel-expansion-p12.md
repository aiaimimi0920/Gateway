# 服务商分渠道扩展：第 12 批，Qwen Key 保存的受众安全前置

> 2026-11-02；R09 / W05 与 P01 的无凭据切片，定向离线验证通过；不是普通 Qwen 模板、W05 或总目标完成。
> `main`，HEAD `7969f12c85efceacf2a5e384e143b5c39a550981`。未提交、推送、打包或变更运行实例；总目标未完成。

## 1. 为什么本批先补保存边界

接续 [P11 运行底座](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/docs/progress/2026-11-02-provider-channel-expansion-p11.md)，原准备接普通 API preset、最终配置编译与正常地域/workspace 表单。
实际读取 Console SecretPatch owner 后发现：普通 `keep` 只按稳定 provider/credential ID 找旧材料，没有将 Qwen Key 绑定到原产品或 workspace/地域。
纯 keep 不要求 Secret Grant；因此结构合法的新目标即使能通过运行时校验，也不证明用户授权旧 Key 改投这个目标。
在此边界补好前不提前开放普通 Qwen 模板，编译期/运行期校验不能代替保存时的 old→new 授权边界。

普通 API、Coding Plan、Qwen Web 保持不同产品与材料池；内部 `qwen_workspace_api` marker 仍只是普通 API 产品身份，实际认证仍是 Bearer API Key，不是新认证类型或新额度渠道。
同平台有 API Key/OAuth 也仍须评估 Web 的可用额度；未知关系不合池、不以同名模型证明共享权益。

## 2. 实现与安全边界

- [Qwen 产品识别](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/qwen_workspace.rs)：原始 `qwen-official-api` preset、有效 auth marker 或精确六区 workspace endpoint 任一信号命中即保护，不从可改 Provider ID 推断；credential 采用有效 override/继承值。
- [共享 keep owner](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/keep_bindings.rs)：从原 Bedrock owner 提取 pending、稳定消费者与完整目的地比较；Bedrock/Qwen 只有各自产品 predicate，不建立通用策略配置框架。
- [Bedrock predicate](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/bedrock.rs) 保留既有显式认证模式识别；[共同目的地投影](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets/bedrock_context.rs) 的原字段与比较语义不变，仅说明共享用途。
- [普通 resolver](C:/Users/Public/nas_home/AI/GameEditor/Neuro/Gateway/src/console/secrets.rs) 仍沿原 pointer schema、稳定身份、分类、预算和 keep/replace/clear，在返回编译/持久化前统一校验；不另造秘密写入口、不改变 Grant 或 CAS 协议。

old 或最终 new 是受保护产品时，非空 kept Key、auth/refresh token、敏感 header/body、keepalive 材料不能静默换 endpoint、产品、auth、flow、协议/path、session/runtime、refresh 或 keepalive 目标。
provider 级材料还检查**所有**实际继承消费者：包括新账号、disabled 账号、非 Qwen endpoint 的账号；既有稳定 ID 也不能删除自己的 override 后借用旧 provider 材料。
明确 replace/clear 仍走原 Secret Grant，不把“持有 Grant 的 keep”自动当成 replace。

### 2.1 最终继承关系不能在 redacted draft 上冻结

stage 仅按 old/new provider 或其任一 credential 存在产品信号登记保守候选，不用 redacted placeholder 判继承。
所有补丁解析后，在 resolved document 上重新确认真正受保护的 owner 与逐字段继承关系。

这既避免新账号随后 replace 自己的 Key 仍被误当共享消费者，也修补一个源码检查确认原 Bedrock stage 同样存在的角落：generic parent 的 kept header/body 秘密，可能在 credential 新进入受保护模式、同时 clear 私有 map override 后才产生继承；在 redacted map 上提前过滤会漏掉该消费者。
不受保护的 generic 材料不会因一个使用私有 Key 的独立新 Qwen 账号而自动改变政策。

逐字段“旧消费者存在且原来继承”的检查在去重 cache 外；只对同一稳定 owner pair 的完整、与秘密字段无关的目的地投影比较去重。
label、账号名、模型列表/映射/目录和测试策略等无关元数据不纳入受众投影；稳定 provider/credential 重排可继续 keep。

本批没有新后台任务、网络、锁、进程、存储字段、版本迁移、依赖或 lockfile 变更。
pending 路径由既有 4096 patch、总路径字节及文档字节预算限制；不把秘密值复制到目的地投影或错误中，失败不返回部分候选、不修改 active。

## 3. 验证记录

证据目录：[P12](C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-provider-expansion-20261102-p12)。所有命令将 TEMP/TMP/TMPDIR 置于本批 `runtime-temp`；测试仅用合成材料，不连接上游或读取真实状态。

- 初始 `red-secrets.log`：第一个 target 实际 2 pass / 6 fail；Cargo 默认遇 target 失败停止，第二个 target 未执行，不能算成已运行。
- 扩充 map-clear 与非误伤测试后，`red-secrets-v2.log` 使用 `--no-fail-fast`：消费者合同 3 pass / 7 fail，产品目的地合同 2 pass / 3 fail，合计 5 pass / 10 fail，均已实际运行，不是编译失败。
- 初轮绿测 `green-secrets-v1.log`：新增 Qwen 两组 15/15；Bedrock secret/keepalive/management/credential-mode 共 21/21，合计 36/36；编译 5m33s。后续追加旧侧产品退出、generic 新消费者、跨字段去重及真实 Qwen management 合同，不用这次结果冒充最终扩充集已通过。
- 真实 Qwen management 合同使用既有 raw workspace＋marker 配置，不使用尚未注册的新 preset；它验证真实 router→管理认证→SecretPatch→CAS/commit，不证明新模板创建或真实推理。
- `default-final-v1.log`：扩充后的 Qwen secret/consumer/management 合同 **20/20**；Bedrock 四组回归 **21/21**；共享 management、management-sensitive、site-health-secrets **19/19**，共 **60/60**，编译 1m01s，无 fail/ignore/filter。map-clear 回归的绿测实际遍历 Qwen/AWS 与两类 map；其旧实现红测首个 Qwen 分支即失败，不将它声称为旧 Bedrock 的独立运行红测。
- `console-document-regression.log`：**64 pass / 0 fail / 1 ignored / 1 filtered**；ignored 为需要显式导出前端合成 fixture 的 opt-in Session 回环；filtered 为前批已知未修的 `legacy_compiler_rejects_extreme_token_expiry_without_panicking`，命令明确 `--skip`，不算成通过。默认配置定向总计 **124 pass**，不是全仓回归。
- 只读独立复审已检查 old/new predicate、conservative stage、resolved 继承、全部消费者及 checked cache：未发现新的绕过或原 Bedrock 保护放宽；这不替代运行结果。
- `checker-v1.log`：checker **31/31**；`ratchet-v1.log` exit 0，3961 文件、14 immutable runtime artifacts、governed source >700=0。使用同一 checker 的无报告写入入口，未覆盖原有脏有效行报告。
- `feature-disabled-final.log`：`--no-default-features --features line-gemini-web-reverse` 下，Qwen 两组纯 resolver 合同 18/18、Bedrock secret/keepalive 13/13，共 **31/31**，编译 5m32s。Qwen/Bedrock API line 与 common families 均关闭；Gemini Web 是前批沿用的最小编译底座，不宣称全 feature 关闭或所有组合矩阵通过，也未修改 Gemini lane。
- 默认 124 pass，加关闭组合重复执行 31 pass，共 **155 次 Rust 定向通过执行**，不是 155 个独立用例；checker 31 项单列，不混进 Rust 数量。
- `source-before.json` 记录原 8 文件 scope；后加 management 合同的不存在状态另记在 `source-before-addendum-v1.json`，不覆盖原 before。`source-frozen-v1.json` 冻结最终 9 个源码文件，`source-verify-final.log` 确认匹配，最大 369 有效行。
- `format-final.log` 通过；`p11-source-final.log` 证明 P11 冻结的 13 源码文件未漂移，不用来源匹配代替重新运行其全部 wire 测试。`diff-final.log` 为全工作树 `git diff --check` 通过；未撤销其他脏改，Cargo.toml/Cargo.lock 无改动，收尾无 Cargo/rustc 残留。
- `document-final.log` 记录最终 4 文档 UTF-8 无 BOM、围栏、本地引用及 13 scoped tracked/untracked whitespace 检查；没有前端源码变更，不运行假 UI 构建或发布包。

有效代码行按官方 lexer 测量；无 >500 新文件、例外、baseline 或 immutable policy 变更：

| 文件 | 本批前 → 后 |
| --- | ---: |
| `src/console/secrets.rs` | 367 → 369 |
| `src/console/secrets/bedrock.rs` | 166 → 11 |
| `src/console/secrets/bedrock_context.rs` | 70 → 70 |
| `src/console/secrets/keep_bindings.rs` | 新增 171 |
| `src/console/secrets/qwen_workspace.rs` | 新增 21 |
| `tests/qwen_console_secret_contract.rs` | 新增 156 |
| `tests/qwen_console_keep_consumers_contract.rs` | 新增 224 |
| `tests/qwen_console_management_contract.rs` | 新增 174 |
| `tests/qwen_console_secret_contract/support.rs` | 新增 35 |

## 4. 本批没有完成的内容与接力入口

本批**没有注册普通 API preset、开放模板或正常 UI**；W05 仍 `in_progress`，不把保存安全前置当成完整产品。

下一批继续：

1. 新 preset `qwen-official-api`、profile 映射与 feature gate；按原始 preset 锚定，校验每个最终可执行 credential 的有效 payload。
2. 按 `uses_provider_default_account()` 区分真正默认账号和非执行 provider 模板；显式空池/凭据池的抽象 provider 允许由 credential 覆盖目标，但不能为通过配置编译而放松 P11 runtime fail-closed。
3. 正常创建与账号 override 的六区 region/workspace 输入，无默认地域、无自动账号/模型/路由；保存纯 owner 与 UI 使用同一目标校验。继承值重开不展平为 override。
4. 目标变化与 Key 受众变化使用明确 replace/clear；旧模型目录不能作为新目标的同一账号证据。已有父级 provider 目标的正常编辑入口仍需实现，不能以高级 JSON 编辑冒充完成。
5. 动态目录 management/Grant/slot/精确 credential、source+accountBaseUrl 前端 schema/CAS/fences、精确 probe，以及真正生产 pipeline buffered/stream/响应/取消合同。

Bedrock 普通参数/reasoning/signature/取消 audit、R01–R16 其余 OAuth/Web/API/媒体、P50 devtest、集中录制与逐渠道真实验收继续保留。
没有使用历史凭据、产生真实上游费用、录制、验收标蓝或交付新 EXE；没有全仓回归结论。
