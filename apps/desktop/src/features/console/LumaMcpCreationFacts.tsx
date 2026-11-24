import { Button, CopyButton } from "@neuro/ui";
import type { LumaCreationTask } from "../../api/console/luma-creation";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

export function LumaMcpCreationFacts({ task, onReuse }: { task: LumaCreationTask;
  onReuse?: (assetId: string, operation: "generate" | "edit" | "reframe") => void;
}) {
  const { t } = useUiLocale();
  return <div className="n-stack" aria-label="Luma MCP">
    <div>{t("MCP 任务 ID：", "MCP task ID: ")}<code>{task.id}</code>
      <CopyButton compact label={t("复制 MCP 任务 ID", "Copy MCP task ID")} onCopy={() => navigator.clipboard.writeText(task.id)} /></div>
    {task.requestedModel && <p>{t("请求模型：", "Requested model: ")}{task.requestedModel}</p>}
    {task.workspaceBinding && <p>{t("素材来源任务：", "Source task: ")}{task.workspaceBinding.taskId}</p>}
    <p>{t("会话费用：", "Session cost: ")}{task.usage ? `${task.usage.credits} credits · ${task.usage.settled ? t("已结算", "Settled") : t("未结算", "Unsettled")}` : t("未知", "Unknown")}</p>
    {task.assets.map(asset => <div key={asset.id}>{asset.state === "ready" && asset.url && /^https:\/\//i.test(asset.url)
      ? <a href={asset.url} target="_blank" rel="noreferrer noopener">{t("打开生成结果", "Open generated asset")}</a> : t("暂无素材链接", "No asset link")}
      <CopyButton compact label={t("复制素材 ID", "Copy asset ID")} onCopy={() => navigator.clipboard.writeText(asset.id)} />
      {asset.state === "ready" && onReuse && <div className="nt-actions">
        <Button onClick={() => onReuse(asset.id, "edit")}>{t("编辑此素材", "Edit this asset")}</Button>
        {asset.kind === "image" && <Button onClick={() => onReuse(asset.id, "generate")}>{t("用作参考图片", "Use as reference image")}</Button>}
        {asset.kind === "video" && <Button onClick={() => onReuse(asset.id, "reframe")}>{t("扩画此视频", "Reframe this video")}</Button>}
      </div>}
    </div>)}
  </div>;
}
