import { Check, Pencil, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { TranslateFn } from "./accountsLedgerTypes";

type Props = {
  providerLabel: string;
  label: string;
  value: string;
  secret?: boolean;
  configured?: boolean;
  numeric?: boolean;
  minimum?: number;
  maximum?: number;
  disabled: boolean;
  discardDraft?: boolean;
  unavailableReason?: string;
  description?: string;
  onEdit?: () => void;
  onSave?: (value: string) => boolean | void;
  t: TranslateFn;
};

/** Drafts are private to the open editor; cancel/explicit lock never retains a password. */
export function ProviderLifecycleField({ providerLabel, label, value, secret = false,
  configured = false, numeric = false, minimum = 0, maximum = Number.MAX_SAFE_INTEGER,
  disabled, discardDraft = false, unavailableReason, description, onSave, onEdit, t,
}: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [savedSecret, setSavedSecret] = useState(false);
  const [error, setError] = useState("");
  const saving = useRef(false);
  const restoreFocus = useRef(false);
  const editButton = useRef<HTMLButtonElement>(null);
  const close = () => {
    restoreFocus.current = true;
    setEditing(false);
    setDraft("");
    setError("");
  };
  useEffect(() => {
    if (!editing && restoreFocus.current) {
      restoreFocus.current = false;
      editButton.current?.focus();
    }
  }, [editing]);
  useEffect(() => {
    if (discardDraft) {
      setEditing(false);
      setDraft("");
      setError("");
    }
  }, [discardDraft]);
  const save = () => {
    if (disabled || !onSave || saving.current) return;
    if (numeric && (!/^\d+$/.test(draft) || !Number.isSafeInteger(Number(draft)) ||
      Number(draft) < minimum || Number(draft) > maximum)) {
      setError(t(`请输入 ${minimum} 至 ${maximum} 之间的安全整数。`, `Enter a safe integer between ${minimum} and ${maximum}.`));
      return;
    }
    if (secret && draft.length === 0) return;
    saving.current = true;
    try {
      if (onSave(draft) !== false) {
        if (secret) setSavedSecret(true);
        close();
      }
    } finally { saving.current = false; }
  };
  const display = secret ? (configured || savedSecret ? "••••" : "—") : value || "—";
  return (
    <div className={`nt-provider-lifecycle__field${editing ? " nt-provider-lifecycle__field--editing" : ""}`}>
      <span className="nt-provider-lifecycle__label" title={description ? `${label}: ${description}` : label}>{label}</span>
      {editing ? <>
        <input autoFocus className="nt-input nt-provider-lifecycle__field-input"
          type={secret ? "password" : numeric ? "number" : "text"}
          autoComplete={secret ? "new-password" : "off"}
          min={numeric ? minimum : undefined} step={numeric ? 1 : undefined}
          inputMode={numeric ? "numeric" : undefined}
          aria-label={`${providerLabel} ${label}`} aria-invalid={Boolean(error)}
          value={draft} disabled={disabled}
          onChange={(event) => { setDraft(event.currentTarget.value); setError(""); }}
          onKeyDown={(event) => {
            if (event.key === "Escape") { event.preventDefault(); close(); }
            if (event.key === "Enter") { event.preventDefault(); save(); }
          }} />
        <button className="nt-icon-action" type="button" aria-label={t(`保存 ${providerLabel} ${label}`, `Save ${providerLabel} ${label}`)}
          disabled={disabled || (secret && !draft)} onClick={save}><Check size={14} /></button>
        <button className="nt-icon-action" type="button" aria-label={t(`取消编辑 ${providerLabel} ${label}`, `Cancel editing ${providerLabel} ${label}`)}
          onClick={close}><X size={14} /></button>
      </> : <>
        <span className="nt-provider-lifecycle__field-value" title={secret ? undefined : display}>{display}</span>
        <button ref={editButton} className="nt-icon-action" type="button"
          aria-label={t(`编辑 ${providerLabel} ${label}`, `Edit ${providerLabel} ${label}`)}
          title={unavailableReason || t("编辑并保存到路由草稿", "Edit in the route draft")}
          disabled={disabled || (!onSave && !onEdit)} onClick={() => { if (onEdit) { onEdit(); return; } setDraft(secret ? "" : value); setError(""); setEditing(true); }}>
          <Pencil size={14} />
        </button>
      </>}
      {error ? <span role="alert" className="nt-provider-lifecycle__field-error">{error}</span> : null}
    </div>
  );
}
