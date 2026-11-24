# SenseNova Token Plan：包内双后端自动探测

复用 `C:/Users/Public/nas_home/AI/GameEditor/Neuro/release/Gateway/gateway-devtest-sensenova-catalog-20261122-231803`，未重新打包或改动包。manifest SHA-256 为 `9e424647f2f61beb341ab60b12605d7d860407f3fbf92d88049869224b5955bc`，后端 EXE SHA-256 为 `12ea298034cc223ae565debeedff9f3bc288427165d22f4311383f83a47dc3bf`。

- 两个后端共享专用 PostgreSQL／Redis，两个真实调度周期各向不转发的合成 TLS peer 发出一次非流式 Chat，没有手动探测或公共推理。固定 authority、端点、模型和请求参数均匹配。
- 在整个独立 namespace 恰有两条内部完成审计：产品／账号／模型归属正确，HTTP 200、输入／输出／总量为 2／3／5，零公共审计。重启读回、Redis 活跃槽、缺 CA 启动拒绝及正常 drain 均通过。
- 增强审计断言后，同包串行复跑 SenseNova 与 Command Code，两者均 `status=passed`。夹具单元测试 6/6，有效行 checker 31/31 与 ratchet 通过。

回执：

- SenseNova：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-sensenova-scheduler-be36223a014946b4bb9f3584832d6b11/result.json`
- Command Code：`C:/Users/Public/nas_home/AI/GameEditor/linshi/gateway-packaged-provider-fixture/published-commandcode-f13a760cc4f941a494c2acfbd75cd718/result.json`

复跑命令见 [夹具说明](../../tools/packaged-provider-fixture/README.md)。此验收不涵盖真实 Key／订阅／费用，不证明视频或控制台 OAuth 能力，未部署。
