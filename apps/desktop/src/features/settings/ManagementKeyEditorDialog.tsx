import { useRef, type RefObject } from "react";
import { Dialog, Notice } from "@neuro/ui";
import { useUiLocale } from "../../i18n/UiLocaleProvider";
import { ManagementKeyEditor } from "./ManagementKeyEditor";
import type { ManagementKey } from "./managementKeysApi";

/** Editors are modal so key count, validation and drafts cannot reflow the settings rows. */
export function ManagementKeyEditorDialog({ open, entry, locked, error, onCancel, onSave, returnFocusRef, resolveReturnFocus }: {
  open: boolean; entry?: ManagementKey; locked: boolean; error: string;
  onCancel(): void; onSave(name: string, token: string): void;
  returnFocusRef: RefObject<HTMLElement | null>; resolveReturnFocus(target: HTMLElement | null): HTMLElement | null;
}) {
  const { t } = useUiLocale();
  const input = useRef<HTMLInputElement>(null);
  return <Dialog open={open} onOpenChange={value => { if (!value && !locked) onCancel(); }} busy={locked} dismissOnOutside={false}
    title={entry ? t("编辑管理密钥", "Edit management key") : t("添加管理密钥", "Add management key")}
    closeLabel={t("关闭", "Close")} className="nt-management-key-editor-dialog" initialFocusRef={input}
    returnFocusRef={returnFocusRef} resolveReturnFocus={resolveReturnFocus}>
    <ManagementKeyEditor key={entry?.id ?? "new"} entry={entry} locked={locked} initialFocusRef={input} onCancel={onCancel} onSave={onSave} />
    {error && <Notice tone="danger">{error}</Notice>}
  </Dialog>;
}
