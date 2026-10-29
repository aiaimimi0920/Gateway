# 凭据池接入方式筛选与名称后缀

## 行为

- 在站点排序右侧增加 oauth、api、web 原生 checkbox，默认全选，支持任意组合和全部取消。
- 接入方式筛选与搜索、排序叠加；筛选变更重置增量加载与滚动位置，不改变服务商调度开关、优先级或 route-config。
- 分类读取 preset、protocolProfile、adapter、manualAddFamily 等公开元数据，不读取账号密钥，不以用户自定义标签判断接入方式。
- 卡片正反面标题和旧台账服务商标题追加匹配的后缀；已有后缀不会重复。内置 OAuth 的历史 chatgpt 标签显示为 codex-oauth。
- 仅改变此页面展示名称，内部 ID、存储标签、账号归属、路由引用不迁移。其他管理弹窗继续保留原始标签。
- 权益组与模型池不增加这三个筛选控件。控件使用现有 Neuro 信号色，支持键盘操作和自动换行。

## 验证

- 新增分类测试先红，再实现分类、筛选与标题展示。
- 六个直接相关测试文件 26/26 通过，含 BrowserConsoleApp 实际组件集成、checkbox 键盘、多选、空选、卡片与增量加载测试。
- 相邻账号库等测试首轮 26/27，一处旧标题断言需添加 api 后缀；修正后该文件 9/9 通过。
- 首轮并发测试有一项 5 秒超时；降低到 2 个测试 worker 后原测试通过，未放宽超时标准。
- TypeScript typecheck、effective-lines checker 31/31、ratchet、git diff --check 通过。
- 没有编译或启动新 EXE，没有更新用户运行实例，没有真实浏览器/原生窗口的视觉验收；本记录不把组件测试等同于运行包验收。

有效代码行：ProviderPoolToolbar 49→60、providerPoolView 48→50、AccountsLedgerWorkspace 413→414、ProviderLedgerCard 293→294、usePoolScrollLoading 63→65；新增 providerAccessKind 22 行。无新依赖、外部请求、凭据写入或数据库迁移。保持其他线程已有修改。
