import { useEffect, useId, useRef, useState } from "react";
import type { TranslateFn } from "./accountsLedgerTypes";
import type { useCredentialTestDraft } from "./useCredentialTestDraft";

/** Compact plan identity and target picker; the popup never changes the form's height. */
export function CredentialTestPlanHeader({ name, onNameChange, state, locked, onChange, t }: {
  name: string; onNameChange(name: string): void; state: ReturnType<typeof useCredentialTestDraft>;
  locked: boolean; onChange(): void; t: TranslateFn;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const allCheckbox = useRef<HTMLInputElement>(null);
  const id = useId();
  const kind = state.selection.kind === "account" ? "account" : "subpool";
  const options = state.options.filter((option) => option.scope.kind === kind);
  // Preserve legacy pool scopes, including ungrouped and future accounts, until a subset is chosen.
  const wholePool = state.selection.kind === "pool";
  const keys = wholePool ? options.filter((option) => state.targets.some((target) => target.providerId === option.scope.providerId)).map((option) => option.key) : state.selection.keys;
  const selected = options.filter((option) => keys.includes(option.key));
  const all = wholePool ? state.targets.length === state.options.filter((option) => option.scope.kind === "pool").length
    : options.length > 0 && selected.length === options.length;
  const label = kind === "subpool" ? t("选择子池", "Select subpools") : t("选择账户", "Select accounts");
  useEffect(() => {
    if (allCheckbox.current) allCheckbox.current.indeterminate = !all && selected.length > 0;
  }, [all, selected.length, open]);
  useEffect(() => {
    if (!open || locked) return;
    allCheckbox.current?.focus();
    const outside = (event: PointerEvent) => { if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false); };
    // Radix dialogs listen during document capture; consume Escape first so only this popup closes.
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault(); event.stopPropagation(); setOpen(false); trigger.current?.focus();
    };
    document.addEventListener("pointerdown", outside);
    window.addEventListener("keydown", escape, true);
    return () => { document.removeEventListener("pointerdown", outside); window.removeEventListener("keydown", escape, true); };
  }, [open, locked]);
  useEffect(() => { if (locked) setOpen(false); }, [locked]);
  const selectAll = (checked: boolean) => {
    if (checked && kind === "subpool") state.changeTargets("pool", state.options.filter((option) => option.scope.kind === "pool").map((option) => option.key));
    else state.changeTargets(kind, checked ? options.map((option) => option.key) : []);
    onChange();
  };
  return <div className="nt-test-plan-header">
    <label className="nt-test-plan-identity"><span>{t("计划", "Plan")}</span>
      <input className="nt-input" aria-label={t("计划名称", "Plan name")} value={name} maxLength={256} disabled={locked}
        onChange={(event) => { onNameChange(event.target.value); onChange(); }} /></label>
    <div className="nt-test-plan-targets">
      <select className="nt-input" aria-label={t("计划类型", "Plan type")} value={kind} disabled={locked}
        onChange={(event) => { state.changeKind(event.target.value === "account" ? "account" : "subpool"); setOpen(false); onChange(); }}>
        <option value="subpool">{t("子池类", "Subpools")}</option><option value="account">{t("账户类", "Accounts")}</option>
      </select>
      <div ref={root} className="nt-test-plan-picker" onBlur={(event) => { if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false); }}>
        <button ref={trigger} type="button" className="nt-input nt-test-plan-picker-trigger" disabled={locked}
          aria-label={label} aria-expanded={open && !locked} aria-controls={id} onClick={() => setOpen(!open)}>
          <span>{all ? t("全选", "All selected") : selected.length ? t(`已选 ${selected.length} 项`, `${selected.length} selected`) : label}</span><span aria-hidden="true">▾</span>
        </button>
        {open && !locked ? <div id={id} className="nt-test-plan-picker-menu" role="group" aria-label={label}>
          <label><input ref={allCheckbox} type="checkbox" checked={all} disabled={kind === "account" && !options.length}
            onChange={(event) => selectAll(event.target.checked)} /><span>{t("全选", "Select all")}</span></label>
          {options.map((option) => <label key={option.key}><input type="checkbox" checked={keys.includes(option.key)}
            onChange={(event) => { state.changeTargets(kind, event.target.checked ? [...keys, option.key] : keys.filter((key) => key !== option.key)); onChange(); }} />
            <span>{option.label.replace(/^(子池|账户) · /, "")}</span></label>)}
          {!options.length ? <p className="nt-copy">{kind === "subpool" ? t("暂无子池；全选覆盖整个凭据池", "No subpools; select all covers the pool") : t("暂无账户", "No accounts")}</p> : null}
        </div> : null}
      </div>
    </div>
  </div>;
}
