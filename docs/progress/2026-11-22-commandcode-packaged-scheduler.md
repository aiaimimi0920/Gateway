# Command Code：包内双进程定时探测与原生账号补验

## 包内定时探测

- `desktop / devtest`：`C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-commandcode-scheduler-20261122-222437`。暂存及最终包校验均通过，6169 个载荷文件；最终 `gateway.exe` SHA-256 为 `39507f4500ccadf6691f0b69beef32e3634d5173cc0804f5684b2469fa57b6c7`，`gateway-ui.exe` 为 `5362f3c1bcfc7aa8fb3ce765dc3ca26aa5531875ad28da12121f91ed49088d18`。
- 两个独立 `gateway.exe` 通过专用 PostgreSQL／Redis 跑真实定时周期，不手动触发、不发公共请求。合成 TLS peer 恰好收到两次固定的非流式 Chat；数据库恰好两条 `completed` 内部审计，每条 HTTP 200 和 `2/3/5` tokens，公共审计为零。重启后仍读到同一评估和有效 Redis 槽，未多发；三次启动正常 drain，专用容器已停止，保留数据。
- 使用合成 Key、独立根证书和不转发的 loopback TLS peer；旧包缺少接线时在启动保护步骤失败、零请求，新包缺 CA 直接拒绝。新包第一次运行通过缺 CA 门禁、合成 peer 到达一次，后因夹具沿用空路由场景的 `redis_required=true` 断言而退出。Standalone 有独立静态路由时按生产合同报告 `redis_required=false`，健康 Redis 探测仍须为 `true`；只修夹具断言，复用同一发布包完整重跑通过。
- 通过回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-commandcode-39f539ea394c4126a8f9cee54d4ca9ed/result.json`。后端和 manifest 哈希均与最终包一致。复跑入口见 [包级夹具说明](../../tools/packaged-provider-fixture/README.md)。
- Rust 定向 **105 通过／0 失败／7 忽略**；默认功能 `cargo check --lib --bins --tests`、Python TLS 和诊断 **5/5**、56 项功能线清单、格式、有效行 checker **31/31** 和 ratchet 通过。7 个 opt-in 源码用例未在这条命令中运行；包级 runner 是单独证据。

## 原生桌面账号：添加、保留 Key 编辑、重启

- 复用[此前校验过的桌面包](2026-11-22-commandcode-server-probe-audit.md#桌面手测包)，重新核验两份 EXE 的 SHA-256。使用新隔离数据根、随机 loopback 端口与该包运行的原生 WebView2 控制台，没有通过脚本直接写管理 API。
- 从空池添加 Command Code 合成账号和手动模型，通过 UI 编辑手动模型而不重输 Key。两次表单 `PUT` 及后续 `GET` 均 HTTP 200；响应 `configured=true`，编辑控件不回填 Key，响应／后端日志均不回显合成 Key。未点击拉取模型、手动探测或公共生成。
- 退出后从相同隔离数据根启动第二个原生窗口，账号列表、编辑后的模型和掩码保持；本轮两个窗口及托管后端均已退出，API 和调试端口已释放。回执：`C:/Users/Public/nas_home/AI/GameEditor/linshi/commandcode-native-d542e495bb6e4dfdb2b343a558009e18/result.json`。

上述是合成账号桌面流程和内部定时探测的两份独立证据，不替代真实账号目录／调用／费用、批准范围内其他协议或实际部署验收。
