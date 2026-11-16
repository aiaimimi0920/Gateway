import { Button, Checkbox, Dialog, EmptyState, Notice } from "@neuro/ui";
import { useEffect, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { CredentialPruneReview } from "./useCredentialPruneReview";
import "./CredentialPruneDialog.css";

type Props = { review: CredentialPruneReview | null; locked: boolean; onClose(): void; onApply(ids: string[]): Promise<void> };

export function CredentialPruneDialog({ review, locked, onClose, onApply }: Props) {
  const { t } = useUiLocale();
  const [selected, setSelected] = useState<string[]>([]);
  useEffect(() => { setSelected([]); }, [review?.providerId, review?.preview?.digest]);
  if (!review) return null;
  const labels: Record<string, string> = {
    permanent_auth_failure: t("永久认证失效", "Permanent authentication failure"),
    account_deleted: t("账号已删除", "Account deleted"),
    permanent_upstream_rejection: t("永久拒绝", "Permanent upstream rejection"),
    archived_and_removed: t("已归档并移除", "Archived and removed"),
    archive_failed: t("归档失败，已保留", "Archive failed; retained"),
    archive_budget_exhausted: t("归档预算用尽，已保留", "Archive budget exhausted; retained"),
    retained_revision_changed: t("配置已变化，已保留", "Configuration changed; retained"),
    commit_unconfirmed: t("提交结果未确认，请刷新核对", "Commit unconfirmed; reload to verify"),
  };
  return <Dialog open busy={review.applying} onOpenChange={open => { if (!open) onClose(); }}
    title={`${t("失效号清理", "Invalid credential cleanup")} · ${review.providerId}`}
    description={t("所选账号成功归档后才会从凭据池移除。", "Selected credentials are removed from the pool only after archival succeeds.")}
    closeLabel={t("关闭清理窗口", "Close cleanup dialog")}>
    {review.loading && <Notice tone="info">{t("正在检查…", "Inspecting…")}</Notice>}
    {review.error && <Notice tone="danger">{review.error}</Notice>}
    {review.receipt ? <>
      <Notice tone={review.receipt.removedCount === null ? "warning" : "success"}>{review.receipt.removedCount === null ? labels.commit_unconfirmed :
        t(`已移除 ${review.receipt.removedCount} 个账号。`, `${review.receipt.removedCount} credentials removed.`)}</Notice>
      <ul className="nt-prune-dialog__list">{review.receipt.items.map(item => <li key={item.credentialId}>
        <span>{item.credentialId}</span><span>{labels[item.status]}</span>
      </li>)}</ul>
    </> : review.preview && <>
      <div className="n-dialog-actions">
        <Button disabled={locked || review.applying || !!review.error} onClick={() => setSelected(review.preview!.items.slice(0, 128).map(item => item.credentialId))}>{t("选择前 128 项", "Select first 128")}</Button>
        <Button disabled={review.applying} onClick={() => setSelected([])}>{t("清空选择", "Clear selection")}</Button>
      </div>
      {!review.preview.items.length && <EmptyState compact role="status" title={t("没有可清理的永久失效账号。", "No permanently invalid credentials to remove.")} />}
      <ul className="nt-prune-dialog__list">{review.preview.items.map(item => <li key={item.credentialId}>
        <Checkbox label={item.credentialId} checked={selected.includes(item.credentialId)}
          disabled={locked || review.applying || !!review.error || (!selected.includes(item.credentialId) && selected.length >= 128)}
          onChange={event => setSelected(ids => event.target.checked ? [...ids, item.credentialId] : ids.filter(id => id !== item.credentialId))} />
        <span>{labels[item.classification]}</span>
      </li>)}</ul>
    </>}
    <div className="n-dialog-actions">
      <Button disabled={review.applying} onClick={onClose}>{t("关闭", "Close")}</Button>
      {review.preview && !review.receipt && <Button variant="danger" loading={review.applying}
        disabled={locked || review.applying || !!review.error || !selected.length} onClick={() => void onApply(selected)}>
        {t("归档并移除所选", "Archive and remove selected")} (<span className="nt-prune-dialog__count">{selected.length}</span>)</Button>}
    </div>
  </Dialog>;
}
