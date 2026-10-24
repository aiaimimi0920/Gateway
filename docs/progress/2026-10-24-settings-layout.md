# 设置切换与通栏排版修复（2026-10-24）

## 本轮问题与修改

- 设置 show/hide 原先修改父 WebView 的顶层 location，再由 native 取消导航；同时分类切换
  会销毁、重建本地 child。本轮改为严格限权的 custom-resource fetch，复用一个本地 child，
  避免分类切换触发整页导航和重复加载。远端页面仍然没有 Tauri commands 权限。
- 初始主题、语言在首帧前同步，背景色与 surface token 一致；React 按 revision 确认 ready
  后显示。过期 ready 触发上下文补发，隐藏及销毁状态阻止旧消息重新显示内容。
- 文档/后端切换的 Close 为不可被 resize/hide 覆盖的清理屏障；busy 也绑定可见 revision。
- 通用设置行原先只有 240px 控件列，且其后加载 CSS 覆盖后端地址专用列；两个按钮把输入框
  挤到几乎不可见。现为 label＋弹性内容列，贯穿内容区，以细线分隔并对齐右侧操作。
- 后端方式显示「本机Gateway」「远程Gateway」，保留原有枚举和存储格式。
  地址仍显示真实表单值，未添加无用字段，常规宽度按钮仍与地址同行。
- 删除诊断页“日志策略在此 Gateway 下次启动时生效……”提示；不改变后端策略执行时机。

## 验证

- 前端定向 17 文件 85 项通过，TypeScript 类型检查通过。
- Tauri connection 定向 19 项、内嵌设置 Python 合约 6 项通过。
- 有效行检查器 31 项及 ratchet 通过；未修改 baseline 或 exception。
  本轮 25 个相关文件的严格 UTF-8 无 BOM、逐文件 diff check 通过；最大源码为 320 有效行。
- 真实组件隔离浏览器夹具：1440px/900px 下五分类均通栏，无横向溢出；本机、远端地址
  可见，复制按钮操作前后均为 80px，右边界不移动；中英文与截图复核通过。
  独立 child 内容区在 540px/390px、两种语言和浅色主题下也无横向溢出。
- Windows WebView2 隔离 harness 直接编入本轮 presentation/panel/chrome 源码，分别使用
  loopback HTTP 页面和本地 custom-protocol HTTPS 页面：两者设置资源请求均成功，
  native→native→隐藏→重新显示和 resize burst 仅加载一个 child，设置顶层导航计数均为 0。
  两类源尝试调用本地设置 command 均被拒绝；隔离进程与本轮浏览器/HTTP server 已退出。
  HTTPS 用例验证 WebView2 的 HTTPS origin/CORS/资源协议，不是实际公网 HTTPS 后端验收。
  退出时 WebView2 输出 `Failed to unregister class Chrome_WidgetWin_0. Error = 1412`，
  两次测试均正常退出且结果为 done；未将该 teardown warning 隐藏为零日志结果。

证据目录：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-settings-layout-20261024/`。
浏览器夹具中 iframe 只是 child 几何替身；原生 harness 使用真实 Windows child WebView，
但并非完整产品 EXE 的人工视觉验收。未连接付费上游、未操作真实启动项、凭证、数据库或现有
Gateway 配置；未重新打包产品 EXE，保留其他 dirty changes，未提交或推送。
