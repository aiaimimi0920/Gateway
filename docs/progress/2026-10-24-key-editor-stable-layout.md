# API Key 编辑弹窗：精简文案与稳定布局

## 本轮范围

- 移除标题下重复的密钥名称，不改变名称输入值；删除已选数量、权限说明、额度解释和底部动态填写提示。
- 权益组改为带勾选状态的下拉多选。超过六项显示搜索；保留停用、已移除及最多 128 项的边界。
- 下拉使用 native popover top layer，避免被弹窗内部滚动区裁剪，不占用表单布局。
- 额度限制固定为标签、模式选择器和数字输入框。不限额或保留原额度时禁用数字框，不移除它。
- 保留原有合法性校验、后端能力检查、保存禁用条件、计量单位隔离和错误反馈，不修改 API、凭证、数据库或路由行为。
- 固定控件高度、提交按钮宽度和 footer 反馈区域；长名称截断，真实错误可滚动、聚焦并查看完整 tooltip。

## 实测与修正

- 浏览器测量曾发现长中文权益组名称使自动行高增加约 3px，现以固定 40px 控件高度消除。
- 真实 Radix 弹窗曾先于下拉框处理 Escape，现通过 window capture 消费第一次 Escape，保留编辑弹窗及焦点恢复。
- 1600/900/540/390px 四种视口，64 个选择、搜索、空值、非法值、金额及有效期状态，弹窗、字段及按钮位置与尺寸一致。
- 另检查四种视口的保存中和超长接口错误状态；未调用真实上游。
- 相关 API Key 测试 11 个文件、45 项通过；TypeScript、TypeScript language-service formatter、checker tests 31 项和有效行 ratchet 通过。
- 一次四 worker 并发回归中，既有服务端表单测试触及 5 秒超时；保留原超时配置，以两个 worker 重跑全部 45 项通过。
- 所有本轮源码文件小于 500 有效行；无历史大文件增长，不涉及依赖升级。

## 证据与交付边界

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-key-editor-20261024`。

`browser-geometry.log`、`browser-errors.log` 保存实际浏览器测量结果；`editor-*.png` 为隔离夹具截图，数据不来自真实凭证。
`tests-final.log` 和 `static-final.log` 记录最终定向回归与静态检查。快速手测包沿用官方 `tools/package-gateway-devtest.ps1 -Edition desktop -Jobs 4`，
构建/打包结果以同目录日志、包内 manifest/provenance 和独立哈希核验为准，不以本文代替成功凭据。

未运行正式发布审计、全量 Rust 回归、付费服务商测试；用户原生窗口人工手测仍由用户进行。
不覆盖旧包，不自动启动新包或替换 4200 实例，不创建便携 `.ng`，不提交或推送。
