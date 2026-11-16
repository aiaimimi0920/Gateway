# Copilot 临时账号管理目录

本块在 owned runtime 和显式账号目录之上接入管理 HTTP 入口；不代表 Copilot 整卡完成，
也不关闭原计划的全部工作项。没有新增 provider preset、凭据保存、OAuth 登录或公共推理。

## 入口与部署边界

仅显式启用 `copilot-sdk-runtime` feature 后注册：

```text
POST /v1/internal/gateway/console/copilot-model-catalog
```

请求字段为 `githubToken`、`expectedRevision`，拒绝未知字段。复用管理身份与
`x-secret-grant` 授权，执行前后检查配置 revision，发布前再次检查授权和关闭状态。
请求体上限 32 KiB，响应均为 `Cache-Control: no-store`。
成功响应为 `{revision, catalog: {models: [...]}}`，保留 SDK 模型的 policy、terms、
capabilities 和可选 billing；目录可见不等于推理权限，不自动接受条款。

受信任进程环境必须设置以下两个已存在的绝对路径：

- `GATEWAY_COPILOT_CLI`：运维提供的可信 Copilot CLI EXE。
- `GATEWAY_COPILOT_WORKSPACE_ROOT`：每操作随机隔离目录的父目录。

请求不能覆盖上述路径。没有自动查找、下载、升级或分发 CLI；默认构建不启用此入口。
沿用固定 SDK 1.0.31 的 experimental typed RPC，升级时需重新核验兼容性。

## 生命周期与秘密边界

每次读取创建独立随机目录和 home，不共享账号缓存、不保存 token、不改路由配置、
不发送生成请求。复用 discovery 的两个全局槽位，每 AppState 只允许一个 Copilot
目录任务；忙时拒绝，不新增无界队列。模型最多 4096 项，序列化结果最多 1 MiB。
在解码后的 JSON 字符串及对象键中检查 token，发现回显即拒绝，不修改模型标识符。

任务由 Gateway 后台 owner 持有并 join；HTTP 断连通知后台关闭，不直接丢弃 child owner。
启动预算 15 秒，读取预算 30 秒，stop 外层预算 40 秒，它们不是总计 40 秒的预算。
进程生命周期仍由 SDK 负责，不额外添加独立进程 killer。成功 stop 后才删除该操作的
随机目录；启动或停止未确认时保留现场供人工恢复，不承诺所有异常路径无残留。
API 错误固定脱敏，不返回 SDK source、token 或内部路径。

## 验证记录

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-copilot-management-20261112-231238`。
合成 EXE 新增独立 PID 文件，以便在操作目录清理后核对真实退出；文件不记录 token。
首次 fixture 编译因 `allowed` move 后复用失败，修为借用后独立 crate 测试
7 passed / 0 failed / 0 ignored。

首次根测试完成编译但 3 项失败：无管理身份的测试误期望 403（现有合同实际为 401）；
合成 EXE 对普通路径与 Windows `\\?\` 路径直接做前缀比较，触发夹具断言，导致目录
503 和断连测试等待超时。单独复现确认该断言后，仅统一夹具两侧 canonicalize 和修正
401 预期，没有改变生产鉴权或路径语义。`fixture-r3` 重新通过 7 项测试；
第二次根测试的准入和 JSON 转义秘密回显回归通过。该轮还暴露测试收尾问题：创建
真实 Secret Grant 的 `build_router` 同时启动 SiteHealth worker，测试没有请求 shutdown
就等待全部后台任务，导致断连项 join 超时、目录读取项 join 卡住。操作目录当时已清理，
随后按精确 EXE 路径与 PID 停止卡住的测试进程，该轮保留失败回执，不记通过。
仅修正测试收尾：断连项先确认 Copilot 操作目录已清理，再发 shutdown 让其他 worker
退出；所有 join 增加有界等待。

新增模块有效行：runtime 编排 113、HTTP 入口 97、管理测试 190；接线文件分别为
console/mod 51 → 53、internal_console 440 → 444、management_config 484 → 486、
background owner 115 → 117。没有修改 baseline 或新增超限文件。

### 最终回执（2026-11-13）

- `management-r3`：根 feature 定向编译与测试成功，4 passed / 0 failed / 0 ignored；
  包括两个显式 opt-in 合成进程测试，没有把它们跳过计成功。编译存在 89 条警告，未扩大清理。
- 核对进程证据时发现并行夹具向共享 JSONL 写多段内容会交错；改为每 PID 一个 JSON 文件。
  `fixture-r4` 重新通过 7 项；随后用刚编译、源码未再变动的根测试 EXE 与新夹具运行
  `management-final`，4 项再次通过，未重复昂贵的根编译。
- 最终两批共 9 个合成 child 经 Windows OpenProcess／WaitForSingleObject 核对均已退出，
  不仅依据 future 返回。`audit.json` 另保留本轮可恢复的 25 个 PID 的退出检查，旧共享
  JSONL 的解析问题原样记录；旧夹具 PID 从仍保留的 `child.pid` 补核，不伪造旧日志。
- 官方 formatter、scoped diff、UTF-8 无 BOM、Cargo.lock 与本轮前快照一致检查通过。
  checker 测试 31 passed；最终 ratchet 通过，5493 文件、governed source >700=0。
- 管理测试覆盖：身份／grant 拒绝、stale revision、输入拒绝与 no-store、A/B 目录隔离、
  失败不回显秘密、配置不变、忙时拒绝、HTTP 断连、Gateway shutdown 与清理／join。
  真实 CLI、真实账号、异常 startup/stop 保留目录的运行验收仍未执行。

本块没有访问真实账号、真实 CLI 或付费上游，没有运行实例变更、提交、推送或出包。
