export type PublicSettingsCategory = "appearance" | "network" | "diagnostics" | "backup" | "about";
type Text = readonly [string, string];
export const settingsCategoryTitles: Record<PublicSettingsCategory, Text> = {
  appearance: ["常规", "General"], network: ["网络与共享", "Network & sharing"],
  diagnostics: ["可观测性", "Observability"], backup: ["同步与备份", "Sync & backup"], about: ["关于", "About"],
};
export type SettingsItem = {
  id: string; category: PublicSettingsCategory; title: Text;
  choices?: readonly Text[]; choiceLayout?: "segmented"; input?: string; action?: Text; group?: Text;
};
const offOn = [["关闭", "Off"], ["开启", "On"]] as const;

/** Static presentation metadata only. Never index tokens, addresses, drafts or account names. */
export const settingsCatalog: readonly SettingsItem[] = [
  { id: "theme", category: "appearance", title: ["外观", "Appearance"] },
  { id: "locale", category: "appearance", title: ["语言", "Language"] },
  { id: "text-size", category: "appearance", title: ["文字大小", "Text size"] },
  { id: "ui-font", category: "appearance", title: ["界面字体", "Interface font"] },
  { id: "code-font", category: "appearance", title: ["代码字体", "Code font"] },
  { id: "provider-name", category: "appearance", title: ["模型名带供应商", "Provider in model names"], choiceLayout: "segmented", choices: [["关闭", "Off"], ["自定义名称不带供应商", "Except custom names"], ["开启", "On"]] },
  { id: "awake", category: "appearance", title: ["防止休眠", "Prevent sleep"], choiceLayout: "segmented", choices: [["关闭", "Off"], ["开启", "On"], ["屏幕也常亮", "Keep screen on"]] },
  { id: "close", category: "appearance", title: ["关闭窗口时", "On close"], choices: [["退出程序", "Exit application"], ["最小化到托盘", "Minimize to tray"]] },
  { id: "startup", category: "appearance", title: ["开机启动", "Auto-start"], choices: offOn },
  { id: "proxy", category: "network", title: ["代理", "Proxy"], choices: [["自动", "Auto"], ["关闭", "Off"], ["自定义", "Custom"]], action: ["测试", "Test"] },
  { id: "endpoint", category: "network", title: ["网关地址", "Gateway address"] },
  { id: "management-keys", category: "network", title: ["面板管理密钥", "Panel management keys"] },
  { id: "lan", category: "network", title: ["局域网共享", "LAN sharing"], choices: offOn },
  { id: "log-directory", category: "diagnostics", title: ["日志保存位置", "Log directory"] },
  { id: "log-retention", category: "diagnostics", title: ["日志保留天数", "Log retention"] },
  { id: "log-limit", category: "diagnostics", title: ["日志占用上限", "History limit"] },
  { id: "diagnostic-export", category: "diagnostics", title: ["导出诊断信息", "Diagnostics"] },
  { id: "log-export", category: "diagnostics", title: ["导出运行日志", "Runtime logs"] },
  { id: "sync", category: "backup", title: ["同步", "Sync"], choices: [["WebDAV", "WebDAV"], ["S3", "S3"], ["GitHub", "GitHub"]], action: ["设置…", "Configure…"] },
  { id: "backup-export", category: "backup", title: ["导出", "Export backup"], action: ["导出…", "Export…"] },
  { id: "backup-import", category: "backup", title: ["导入", "Import backup"], action: ["导入…", "Import…"] },
  { id: "version", category: "about", title: ["版本", "Version"] },
  { id: "release-notes", category: "about", title: ["更新内容", "Release notes"], action: ["查看", "View"] },
  { id: "update-button", category: "about", title: ["更新按钮", "Update button"], choices: offOn },
  { id: "auto-update", category: "about", title: ["自动更新", "Automatic updates"], choices: offOn },
  { id: "update-interval", category: "about", title: ["检查间隔", "Check interval"], choices: [["1 小时", "1 hour"], ["6 小时", "6 hours"], ["24 小时", "24 hours"]] },
  { id: "update-channel", category: "about", title: ["更新渠道", "Update channel"], choices: [["每个版本", "All releases"], ["稳定版本", "Stable"]] },
  { id: "update-download", category: "about", title: ["自动下载更新", "Automatic download"], choices: offOn },
  { id: "update-restart", category: "about", title: ["自动重启更新", "Automatic restart"], choices: offOn },
  { id: "update-source", category: "about", title: ["下载源", "Download source"], action: ["设置…", "Configure…"] },
  { id: "config-directory", category: "about", title: ["配置目录", "Configuration directory"], action: ["打开", "Open"] },
  { id: "website", category: "about", title: ["官网", "Website"], action: ["访问", "Visit"] },
  { id: "license", category: "about", title: ["开源许可", "License"] },
  { id: "community", category: "about", title: ["社区", "Community"] },
];

const searchAliases: Record<string, string> = {
  endpoint: "本机 远程 端口 port backend url", proxy: "代理服务器 system 自动 网络",
  "management-keys": "管理安全 管理员 认证 token key security 登录",
  "log-directory": "文件夹 路径 日志 log folder", "diagnostic-export": "日志 导出 诊断 report",
  telemetry: "遥测 OpenTelemetry OTLP Langfuse 采集 接收方",
  sync: "WebDAV S3 GitHub 云同步", "backup-import": "导入 恢复 restore", "backup-export": "备份 backup",
  close: "桌面 托盘 退出 tray exit", startup: "桌面 开机 自动启动 autostart",
};
export function matchesSetting(item: SettingsItem, query: string) {
  return [...item.title, ...settingsCategoryTitles[item.category], searchAliases[item.id] ?? ""]
    .join(" ").toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
}
