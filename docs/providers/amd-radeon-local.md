# AMD Radeon Cloud Token Factory 本机开发工具

这个客户端仅供本人在本机直接试用和开发。它不是 Gateway 服务商、共享 HTTP 代理、生产部署或账号池，不监听端口、不加载 Gateway 的配置和数据库。先查阅 [AMD 使用条款](https://amd-aim.github.io/radeon-cloud-docs/api/terms/)；不要发送秘密、个人数据或未授权材料。

使用 Python 3.9+，无需安装第三方依赖。在仓库根目录执行：

```powershell
python scripts/amd_radeon_local.py models
python scripts/amd_radeon_local.py usage
python scripts/amd_radeon_local.py chat --model '<目录中的准确 ID>'
python scripts/amd_radeon_local.py chat --model '<目录中的准确 ID>' --stream --max-tokens 64
```

未设置进程环境变量 `RADEON_API_KEY` 时，工具在交互终端隐藏输入密钥。不要把密钥放进命令参数、项目文件、Git 或聊天；使用你自行轮换的新密钥。脚本不保存密钥、自动重试或跟随重定向。固定直连 AMD 官方 HTTPS 地址，不继承代理设置；一次只使用一把 Key。模型目录包含非聊天能力，不代表每个 ID 都可用于聊天；请选择适用于文本聊天的条目。

- [模型目录](https://amd-aim.github.io/radeon-cloud-docs/api/models/)：GET `/models`，按 AMD 的 `data` 数组返回 JSON。
- [聊天接口](https://amd-aim.github.io/radeon-cloud-docs/api/chat-completions/)：POST `/chat/completions`，支持一条文本消息及普通 JSON 或 SSE。首版不运行工具，不接生产，不自动尝试别的模型。
- 离线测试：`python -B -m unittest discover -s scripts/tests -p test_amd_radeon_local.py -v`。全部认证材料和传输响应均为合成夹具，不证明实际账户、权限或额度。

- [每日用量](https://amd-aim.github.io/radeon-cloud-docs/api/usage/)：GET `/usage?include_recent=false`，只输出当天上限、已用、剩余和重置时间。AMD 的字段名虽为 `*_usd`，单位是 Token Factory 积分（pts），不是美元。没有有效数据时报未知，不当成零余额。
