import * as Dialog from "@radix-ui/react-dialog";
import { useEffect, useRef, useState } from "react";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { CredentialPruneReview } from "./useCredentialPruneReview";
import "./CredentialPruneDialog.css";

type Props = { review: CredentialPruneReview | null; locked: boolean; onClose(): void; onApply(ids: string[]): Promise<void> };

export function CredentialPruneDialog({ review, locked, onClose, onApply }: Props) {
  const { t } = useUiLocale();
  const [selected, setSelected] = useState<string[]>([]);
  const focus = useRef<HTMLElement | null>(null);
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
  return <Dialog.Root open onOpenChange={open => { if (!open && !review.applying) onClose(); }}>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay" />
      <Dialog.Content className="dialog-content nt-prune-dialog" aria-describedby="credential-prune-description"
        onOpenAutoFocus={() => { focus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null; }}
        onCloseAutoFocus={event => { event.preventDefault(); focus.current?.focus(); }}
        onEscapeKeyDown={event => { if (review.applying) event.preventDefault(); }}
        onPointerDownOutside={event => { if (review.applying) event.preventDefault(); }}>
        <Dialog.Title>{t("失效号清理", "Invalid credential cleanup")} · {review.providerId}</Dialog.Title>
        <Dialog.Description id="credential-prune-description">{t("所选账号成功归档后才会从凭据池移除。", "Selected credentials are removed from the pool only after archival succeeds.")}</Dialog.Description>
        {review.loading && <p role="status">{t("正在检查…", "Inspecting…")}</p>}
        {review.error && <div className="nt-alert nt-alert--danger" role="alert">{review.error}</div>}
        {review.receipt ? <>
          <p role="status">{review.receipt.removedCount === null ? labels.commit_unconfirmed :
            t(`已移除 ${review.receipt.removedCount} 个账号。`, `${review.receipt.removedCount} credentials removed.`)}</p>
          <ul className="nt-prune-dialog__list">{review.receipt.items.map(item => <li key={item.credentialId}>
            <span>{item.credentialId}</span><span>{labels[item.status]}</span>
          </li>)}</ul>
        </> : review.preview && <>
          <div className="dialog-actions">
            <button className="nt-btn" disabled={locked || review.applying || !!review.error} onClick={() => setSelected(review.preview!.items.slice(0, 128).map(item => item.credentialId))}>{t("选择前 128 项", "Select first 128")}</button>
            <button className="nt-btn" disabled={review.applying} onClick={() => setSelected([])}>{t("清空选择", "Clear selection")}</button>
          </div>
          {!review.preview.items.length && <p role="status">{t("没有可清理的永久失效账号。", "No permanently invalid credentials to remove.")}</p>}
          <ul className="nt-prune-dialog__list">{review.preview.items.map(item => <li key={item.credentialId}>
            <label><input type="checkbox" checked={selected.includes(item.credentialId)}
              disabled={locked || review.applying || !!review.error || (!selected.includes(item.credentialId) && selected.length >= 128)}
              onChange={event => setSelected(ids => event.target.checked ? [...ids, item.credentialId] : ids.filter(id => id !== item.credentialId))} />{item.credentialId}</label>
            <span>{labels[item.classification]}</span>
          </li>)}</ul>
        </>}
        <div className="dialog-actions">
          <button className="nt-btn" disabled={review.applying} onClick={onClose}>{t("关闭", "Close")}</button>
          {review.preview && !review.receipt && <button className="nt-btn nt-btn--danger"
            disabled={locked || review.applying || !!review.error || !selected.length} onClick={() => void onApply(selected)}>
            {review.applying ? t("正在归档…", "Archiving…") : t(`归档并移除所选 (${selected.length})`, `Archive and remove selected (${selected.length})`)}</button>}
        </div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>;
}
