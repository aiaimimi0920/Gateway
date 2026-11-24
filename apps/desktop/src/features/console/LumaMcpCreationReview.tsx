import { useState } from "react";
import { Button, Checkbox, Field, Notice, Textarea } from "@neuro/ui";
import type { LumaCreationTask, LumaReviewAnswer } from "../../api/console/luma-creation";
import { useUiLocale } from "../../i18n/UiLocaleProvider";

type Props = { task: LumaCreationTask; disabled: boolean; onAnswer: (answer: LumaReviewAnswer) => void };
const json = (value: unknown) => JSON.stringify(value, null, 2);
function PlainJson({ value }: { value: unknown }) {
  return <pre style={{ whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{json(value)}</pre>;
}
export function LumaMcpCreationReview(props: Props) {
  const { task } = props;
  const { t } = useUiLocale();
  const page = task.sessionUpdate;
  return <div className="n-stack">
    {page && <section aria-label={t("后续进展", "Follow-up progress")}>
      {typeof page.status_line === "string" && <p>{page.status_line}</p>}
      {page.failure != null && <PlainJson value={page.failure} />}
      {Array.isArray(page.events) && page.events.length > 0 && <PlainJson value={page.events} />}
      {page.has_more === true && <Notice>{t("还有后续内容，请继续查询。", "More content is available. Read again.")}</Notice>}
    </section>}
    {task.review && !task.ended && page?.status !== "ended" && <ReviewForm key={`${task.id}:${task.review.fingerprint}`} {...props} />}
  </div>;
}
function ReviewForm({ task, disabled, onAnswer }: Props) {
  const { t } = useUiLocale();
  const review = task.review!;
  const [raw, setRaw] = useState("");
  const [confirmed, setConfirmed] = useState(false);
  const [sent, setSent] = useState(false);
  let result: unknown;
  let valid = false;
  try { result = JSON.parse(raw); valid = new TextEncoder().encode(JSON.stringify(result)).length <= 16 * 1024; } catch { /* The user is still editing JSON. */ }
  if (valid && review.kind === "select") {
    const options = Array.isArray(review.content.options) ? review.content.options : [];
    const offered = options.map(option => typeof option === "object" && option !== null && "asset_id" in option ? option.asset_id : null);
    const ids = Array.isArray(result) ? result : [];
    valid = Array.isArray(result) && typeof review.content.min === "number" && typeof review.content.max === "number"
      && ids.length >= review.content.min && ids.length <= review.content.max && new Set(ids).size === ids.length
      && ids.every(id => typeof id === "string" && offered.includes(id));
  }
  const receipt = task.responses?.find(item => item.invocationId === review.invocationId);
  const ready = review.show.every(id => review.assets.some(asset => asset.id === id && asset.state === "ready"
    && typeof asset.url === "string" && /^https:\/\//i.test(asset.url)));
  const supported = !!review.invocationId && (review.kind === "review" && !!review.answerFormat
    || ["approve", "confirm", "text_input", "select"].includes(review.kind));
  const canAnswer = supported && ready && !receipt && !sent && task.state === "needs_input" && task.sessionUpdate?.has_more !== true;
  return <section className="n-stack" aria-label={t("待审阅内容", "Pending review")}>
    <PlainJson value={review.content} />
    {review.kind === "confirm" && review.content.destructive === true && <Notice tone="warning">{t("此操作不可撤销。", "This action cannot be undone.")}</Notice>}
    {review.assets.map(asset => asset.state === "ready" && asset.url && /^https:\/\//i.test(asset.url) &&
      <div key={asset.id}>{asset.kind === "image"
        ? <img src={asset.url} alt={t("待审阅图片", "Image for review")} loading="lazy" referrerPolicy="no-referrer" style={{ maxWidth: "100%" }} />
        : asset.kind === "video" ? <video src={asset.url} controls preload="none" style={{ maxWidth: "100%" }} /> : null}
        <a href={asset.url} target="_blank" rel="noreferrer noopener">{t("打开审阅素材", "Open review asset")}</a>
      </div>)}
    {!supported && <Notice tone="warning">{t("当前请求尚不支持在此答复。", "This request cannot be answered here yet.")}</Notice>}
    {supported && <>
      {review.answerFormat && <Field label={t("回答格式与可修改目标", "Answer format and revision targets")}><PlainJson value={review.answerFormat} /></Field>}
      {!ready && <Notice tone="warning">{t("审阅素材尚未就绪，请继续查询。", "Review assets are not ready. Read again.")}</Notice>}
      {(receipt || sent) && <Notice>{t("答复已提交或结果待确认，请查询原任务。", "Answer submitted or unconfirmed. Read the existing task.")}</Notice>}
      {canAnswer && <>
        <Field label={t("审阅答复（JSON）", "Review answer (JSON)")}><Textarea value={raw} maxLength={16384} disabled={disabled}
          onChange={event => { setRaw(event.target.value); setConfirmed(false); }} /></Field>
        {raw && !valid && <Notice tone="warning">{review.kind === "select"
          ? t("请输入候选素材 ID 的 JSON 数组，满足选择数量且不能重复。", "Enter a JSON array of offered asset IDs within the selection limits, without duplicates.")
          : t("请输入有效 JSON，最大 16 KiB。", "Enter valid JSON, at most 16 KiB.")}</Notice>}
        <Checkbox checked={confirmed} disabled={disabled} onChange={event => setConfirmed(event.target.checked)}
          label={t("已审阅内容，确认按此答复继续；可能使用剩余会话 credits", "I reviewed the content and confirm continuing with this answer; remaining session credits may be used")} />
        <Button disabled={disabled || !valid || !confirmed} onClick={() => {
          setSent(true); setConfirmed(false);
          onAnswer({ invocationId: review.invocationId!, fingerprint: review.fingerprint, result });
        }}>{t("提交审阅答复", "Submit review answer")}</Button>
      </>}
    </>}
  </section>;
}
