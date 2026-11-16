import { ConfirmDialog, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import type { ManagementKey } from "./managementKeysApi";

type Props = {
  entry: ManagementKey | null;
  pending: boolean;
  locked: boolean;
  error: string;
  onCancel: () => void;
  onConfirm: (key: ManagementKey) => void;
  resolveReturnFocus: (target: HTMLElement | null) => HTMLElement | null;
};

export function ManagementKeyDeleteDialog({ entry, pending, locked, error, onCancel, onConfirm, resolveReturnFocus }: Props) {
  const { t } = useUiLocale();
  const description = t(`确定删除管理密钥“${entry?.name ?? ""}”吗？删除后该密钥立即失效。`, `Delete management key “${entry?.name ?? ""}”? Access is revoked immediately.`)
    + (entry?.current ? ` ${t("删除当前密钥后将退出登录，请确认已保存另一个有效密钥。", "Deleting the current key signs you out. Keep another valid key.")}` : "");
  return <ConfirmDialog open={entry !== null} onOpenChange={open => { if (!open && !pending) onCancel(); }}
    title={t("确认删除管理密钥", "Confirm management key deletion")} description={description}
    busy={pending} confirmDisabled={locked} confirmLabel={t("确认删除", "Confirm delete")} cancelLabel={t("取消", "Cancel")}
    onConfirm={() => { if (entry) onConfirm(entry); }} resolveReturnFocus={resolveReturnFocus}>
    {error && <Notice tone="danger">{error}</Notice>}
  </ConfirmDialog>;
}
