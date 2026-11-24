# Luma 接手工具与证据迁移

按用户要求，核清 Luma 剩余工作，并将后续持续使用的临时脚本迁入仓库。实际临时目录是 `linshi`，同级 `lingshi` 不存在；本次没有删除任何目录，也没有增加服务商业务功能。

## 改动与长期位置

- 原 `run-check.py` → `tools/luma/run-check.py`：脚本定位仓库根、显式输出参数、隔离 TEMP/TMP/TMPDIR、保留退出码和启动失败，不覆盖旧结果。
- 原 `build-ui.mjs` → `tools/luma/build-ui.mjs`：保留真实 Rsbuild 配置与正常 main；提供合成预览及静态预览构建，移除发布插件、关闭清理、拒绝复用输出目录。修复旧脚本使用 `server.close()` 的层级错误：当前 Rsbuild 返回对象内的 `server.close()` 才是实际关闭入口。
- 原 `preview.tsx` → `apps/desktop/e2e/fixtures/luma-media-preview.tsx`：相对导入实际组件/主题，不发供应商请求，纳入正常 TypeScript 检查。
- 原 `fetch_contracts.py` → `tools/luma/fetch-contracts.py`：固定官方页面集合、支持按页选择、新目录独立抓取、保留时间/状态/原文哈希；失败返回非零。
- [工具说明](../../tools/luma/README.md)与[当前剩余清单](../providers/luma-development-status.md)提供可重复命令和接续边界；总计划明确持续工具入库、临时产物可再生。
- [长期证据](../evidence/luma/README.md)保存 101 份现存历史文件，3,166,074 bytes，逐一核对复制前后 SHA-256；另存本次 20 份迁移检查回执。修正八份近期文档的原临时路径，更新总合同/剩余清单中落后的 MCP 状态。

源码四文件有效行数为 45/69/68/53，均低于 150；原临时脚本带日期路径的测量/快照以及专用堆栈调查不作为后续依赖。旧结果和失败证据保留。没有迁移 `.ng`、凭据、测试数据库、浏览器 profile 或 Web 构建目录。

## 验证与边界

[本次验证索引](../evidence/luma/repository-tools-20261119/index.json)记录实际日志和 SHA-256：

- 新脚本实际构建正常 main 和合成预览成功。
- 预览从非仓库 cwd 启动，loopback HTTP 200，定时关闭退出 0，端口释放。初次关闭错误保留为失败日志，修复后另存成功回执。
- runner 从非仓库 cwd 正确定位仓库；三个临时环境变量一致；故意退出 7 的子进程保留 7，重复输出拒绝覆盖，缺失命令记录 127。
- TypeScript 首轮发现新夹具的类型导入路径错误，修复为实际 `ConsoleRouteDocument` 后通过；Python AST 与 Node 语法检查通过。仓库未配置这些 Python/MJS 工具的统一 formatter，未下载额外依赖。
- 新抓取脚本实际匿名 GET Files 文档 HTTP 200，SHA-256 与已归档快照相同，没有登录或生成调用。
- 检查器 **31 passed**；ratchet **5832 files**、14 immutable runtime artifacts、governed source >700 为 0；`git diff --check` 通过。新文本 UTF-8 无 BOM，新文档 17 个相对链接及归档原文哈希核验通过。

未重新运行未变化的 Rust 产品测试，未生成 EXE、正式发布、提交/推送或修改运行实例/凭据/数据库。旧 Agents 95、Inference 19、前端 108 是原开发批次结果，不当作本轮新测试。

早期临时证据已有 54 处引用目标缺失，见[缺失索引](../evidence/luma/missing-historical-evidence.json)；历史真实账号结果只按既有进展回执记录，本次无法重验原始日志。Luma 持续工具与现存必要证据已脱离旧临时目录；其他项目和 Gateway 其他渠道未做完整清理审计，不能据本次结果宣布整个 `linshi` 可删除。
