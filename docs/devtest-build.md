# 快速手测包与正式发布包

日常默认使用快速手测包，验证功能修复；仅用户明确要求正式发布时走完整 Release。
普通“生成 release 包”请求也按手测处理，除非明确提到正式发布/最终交付。

## 一条命令构建并交付

```powershell
rtk proxy powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/package-gateway-devtest.ps1 -Jobs 4
```

默认生成桌面版，输出到 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-<时间>`。
临时打包位于 `C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-devtest-packages`。
已存在的版本目录会报错，不覆盖、不自动停止运行实例、不携带 `.ng`。

仅构建或预览执行计划：

```powershell
rtk proxy powershell.exe -NoProfile -File tools/build-gateway-devtest.ps1 -Edition desktop -Jobs 4
rtk proxy powershell.exe -NoProfile -File tools/package-gateway-devtest.ps1 -Edition web -Jobs 6 -DryRun
```

`api` 无管理网页，`web` 包含管理网页，`desktop` 另含桌面启动器。
默认并发 `min(4, 逻辑处理器数)`，至少 1，可用 `-Jobs 1..32` 调整；先观察内存再加并发。
Cargo 的 jobs 控制 crate 任务并发，不等于同一 crate 的代码生成并发。

## 编译与缓存

后端及桌面独立工作区均声明 `devtest` profile：`opt-level=1`、`lto=false`、
`incremental=true`、`codegen-units=16`、无调试符号、保留溢出检查。
与正式 `release` profile 相互独立，不改写正式优化设置。

复用已安装的锁定依赖，不做 npm ci，不默认联网审计或升级；缺少依赖直接报错。
Cargo 使用 `--locked`。Web 静态资源每次做一次短构建，避免缓存失效判断漏掉输入；
Rust 后端/桌面由 Cargo 的依赖指纹和增量缓存决定重编，不手工跳过变更判断。
当前 Tauri CLI 没有自定义 profile 开关，因此手测脚本明确执行已配置的 `build:tauri`，
再通过 Cargo devtest 和 `tauri/custom-protocol` 构建嵌入静态资源的桌面程序，沿用
`tauri_build::build()`、配置及图标处理；不启动前端开发服务器，不复制历史 EXE。
若 Tauri 的资源目录或 beforeBuildCommand 改变，脚本会拒绝继续，要求同步维护该流程。

首次 `devtest` 冷构建需要生成独立缓存，可能仍耗时较长；后续同配置构建才体现增量收益。
不承诺未经实测的分钟数。手测包用于功能验证，不用于最终性能基准。

## 来源与验证

构建前后校验源码指纹；变化则拒绝生成凭据。凭据记录类型、profile、并发参数、
时间、源码指纹和 EXE 哈希。打包复用正式清单/哈希校验；类型不符、源码变化或文件
哈希不符均拒绝。手测包无法通过正式打包的类型检查，不能用改名代替正式重编。

日常只跑本次改动关联的检查；包完整性必查。启动相关改动及首次流程验证使用临时
数据目录进行隔离启动。未经授权不跑付费服务商测试，不反复全量回归。

## 正式发布

```powershell
rtk proxy powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/build-gateway-release.ps1 -Edition desktop -UseInstalledDependencies
```

然后使用 `package-gateway-release.ps1`（默认 `BuildKind=release`），按现有正式发布文档
执行安全审查、包/运行验证并报告证据，交给用户做人工回归。严格流程允许有效缓存，
但不得拿 `devtest` 产物冒充正式包。旧凭据缺少 buildKind 时只兼容识别为 release。
