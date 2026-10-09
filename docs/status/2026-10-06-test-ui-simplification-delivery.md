# 测试界面简化与本地手测版交付

## 范围与结论

恢复会话 `01a10c37-e21d-7bb1-80d3-40ce4856a07d` 最后的测试界面调整，
完成剩余交互验收及 copy-only 交付。本次续接没有再次修改生产源码，
没有提交、推送、部署、改写历史或使用真实供应商账户。

用户提出的七项调整已落实：

- 凭据池名称位于左上角，旁边保留“测试 / 结果”导航。
- 删除手动刷新按钮，结果串行自动读取；每轮完成后间隔 5 秒，编辑和执行期间暂停。
- 计划以七列单行展示：名字、测试范围、测试集、自动测试间隔、手动测试、编辑、删除。
- 编辑面板移除自动/手动页签；只配置并保存计划。
- 移除额外的测试账户勾选区；执行覆盖保存范围内的启用账户。
- 编辑器不再显示右侧结果；结果归列表展开区域和汇总页所有。
- 登录后自动取得和续期内存中的短时 Secret Grant，无需重复输入密码。
  管理认证及后端授权检查仍然保留，退出和身份切换使旧回复失效。

## 可运行包

版本：`gateway-product-20261006-test-ui-r1`。

```text
C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-product-20261006-test-ui-r1/gateway-ui.exe
C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/packages/Gateway-gateway-product-20261006-test-ui-r1-windows-x64.zip
```

ZIP SHA-256：`fde7fdd1b56f11f738eb8a61b2d0b047d8509c24e60c28350cfd8a80717ec320`。
保留完整目录运行，不要单独搬出 EXE；既有版本没有覆盖。

本包是 `7431ebbae206fdfe81dd1bf75740c0fc928eb97d` 加 dirty source 的本地手测候选，
不是已发布主干版本。官方构建完成于 `2026-10-06T02:43:35.5202614Z`；
manifest 的 `buildTimestamp` 使用确定性的 Git 时间，不是此次构建的墙钟时间。
构建源指纹为 `671e7191cdd11cb448b7d3955c5f2b732bd6b746359fd30883fc9774d7f4f080`。
当前 159 个 overlay 文件、19 个本轮文件与冻结源码匹配，冻结源码清单为 3413 个文件，
两份 EXE 的 hash 与 provenance 一致。此交付说明是构建后新增文档，不改变二进制。

## 实际验证

- 续接重跑：TypeScript typecheck、20 个测试文件共 100 项测试、31 项行数检查器测试全部通过。
- 当前 Gateway ratchet 通过：2802 个测量文件，14 个分类的不可变 runtime assets，
  governed source 超过 700 行为 0；没有改写基线或覆盖既有行数报告。
- Neuro 通用规范契约及 Gateway / Neuro 根仓库 `git diff --check` 通过。
- 复用并核验同源官方 Windows 构建、包完整性、UI artifacts、SQLite 本地存储、
  原生窗口启动及退出证据；交付后的目录完整性和 ZIP checksum 重新验证通过。
- 实际打包 Web UI + loopback 合成后端：手动连通与智能度测试、启用账户过滤、
  结果展开、模型汇总、计划增删改持久化、自动刷新、390×844 窄屏和键盘焦点通过。
- 自动刷新验证保持结果页不动，外部合成调用后模型 a 的连通度从 100% 更新为 0%，
  调用错误率从 0% 更新为 100%；没有刷新按钮，也没有新增密码输入。
- 已查看宽屏列表、窄屏列表和窄屏编辑器截图。列表在局部横向滚动，外层溢出为 0。
- 683 个其他既有 dirty 文件保持原 hash；原有 544 个缓存取消跟踪记录未动。
- 测试浏览器已关闭，隔离 backend 正常 drain 后退出；精确识别的 helper 已停止，
  本机再次确认测试 PID 和 63771 / 63772 监听均无残留，没有停止其他 Gateway 实例。

## 边界与回执

验收全程未调用真实供应商。智能度是合成测试集评分，不是实际供应商模型能力评级。
首次恢复出现的 `ERR_CONNECTION_REFUSED` 来自已退出的隔离夹具；一次自动刷新断言
超时来自错误的子文本定位器，改用真实 cell textContent 后通过，没有因此改生产源码。
这些事件不应从原始日志中删除，也不应混称产品错误或零错误日志。

本轮不宣称 Issue #3 的历史秘密处置或 GTK 依赖安全阻止项已关闭。
完整证据根目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-test-ui-20261006/`。
关键回执：`continuation-source-verification.json`、`continuation-preservation.json`、
`ui-acceptance-result.json`、`ui-cleanup-result.json`、`delivery-result.json`、
`continuation-final-receipt.json`。
