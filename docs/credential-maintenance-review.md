# 凭据池人工清理合同

本流程将 AIGateway fork 的维护预览、显式选择和逐项结果，用 Gateway 的可信驱动、管理鉴权、归档和路由 revision CAS 重新实现。没有引入第二套凭据存储、后台清理器、数据库版本或删除队列。

## 操作流程

账号库渠道卡片的“删除失效号”现在先请求预览，而不是直接调用旧删除接口。默认不选中任何账号。勾选后，用户执行“归档并移除所选”，结果按账号显示；关闭、查看预览、加载失败都不会触发 apply。

预览仅包括该渠道的有效 credential ID 和受信任驱动返回的永久分类：`permanent_auth_failure`、`account_deleted`、`permanent_upstream_rejection`。余额不足、配额耗尽、临时限流、普通文本中的 401 等不构成此接口的自动判定依据。驱动返回未知 ID、冲突分类、新凭据或非支持分类时失败关闭，不执行清理。

Gateway 预览不修改路由凭据或归档，但会执行已经配置并信任的外部 driver 的 `prune` 分类请求。它不是外部代码沙箱，不能保证该 driver 自身没有外部副作用；驱动应只返回分类。响应不包含 API key、Cookie、driver message 或原始错误。

## 管理 API

两条接口均为 management-only POST，沿用既有管理权限边界，成功响应带 `Cache-Control: no-store`：

```text
/v1/internal/gateway/credential-pool-automation/providers/:providerId/prune/preview
/v1/internal/gateway/credential-pool-automation/providers/:providerId/prune/apply
```

preview 返回 `{ preview: { providerId, revision, digest, archiveRequired: true, items } }`。
apply 请求仅接受：

```json
{
  "expectedRevision": "preview revision",
  "previewDigest": "preview SHA-256 digest",
  "credentialIds": ["explicitly-selected-id"]
}
```

digest 绑定 provider、revision、排序后的 ID / 分类列表，是乐观并发比较值，不是授权令牌、服务端审批记录或不可伪造 ticket。apply 重新运行可信分类，检查 revision、完整 digest 和所选子集；不会信任客户端自报的失效分类。

一池最多检查 1024 个凭据，一次明确选择 1–128 个唯一 ID；apply body 限 64 KiB。driver 沿用现有输出大小和超时限制。检查期间及提交前仍检查 revision，最终写入复用持久化 CAS；独立实例更新不能被旧快照覆盖。

## 归档、提交和不确定性

- 新人工流程始终先归档，即使该池打开了 legacy 永久删除开关。旧直接 `/prune` API 和自动清理行为保留兼容，不宣称它们也自动获得人工预览保障。
- 复用已有本地归档及远端 read-back 验证 owner；仅归档确认成功的所选 ID 进入同一次 route CAS。其他账号不移除。归档副本包含原始凭据，仍由原有本地权限和远端存储安全合同保护，不回传给 UI。
- 归档阶段总预算 60 秒；失败、超时和未开始项保留源凭据。超时可能留下晚到的额外归档副本，不等于文件一定没有写入。
- 本地归档使用 `spawn_blocking`，实际写任务持有同一 provider process lock 和共享 admission。请求取消或等待超时不会提早释放仍在写入的操作锁，避免同池不断累积阻塞写任务。
- 配置变化时，已经完成的归档可以保留，但源凭据不因本次操作移除。purge 仍遵守现有冻结集合与 revision barrier，不声称清空之后晚到的副本也被删除。
- CAS 返回错误不推断“肯定没提交”，回执为 `commit_unconfirmed`、`removedCount: null`。UI 要求刷新核对并重新预览，不自动重放、也不提供原请求盲重试。

逐项 status：`archived_and_removed`、`archive_failed`、`archive_budget_exhausted`、`retained_revision_changed`、`commit_unconfirmed`。整体 outcome：`completed`、`revision_changed`、`commit_unconfirmed`。`completed` 允许部分或全部归档失败，应同时读取逐项结果和 `removedCount`，不能只按 HTTP 200 判断全部成功。

## 界面和验证边界

沿用 Neuro dialog、按钮和语义色；列表内部滚动，长 ID 换行，批量选择最多 128 项。草稿未保存时不能操作。应用中禁用关闭和重复提交；token、API 实例、preview、会话或组件生命周期变化后，迟到结果和旧 callback 不得作用于新的预览。

本批不增加一键恢复 UI、不修改 model cooldown、不解锁全部账号、不调用真实供应商，不更换 live `.ng` 或端口服务。定向健康恢复需要独立处理精确 credential / model / protocol identity 以及健康状态 CAS，不能把本功能当作 E02 已完成。
