import { Check, ChevronDown, Copy } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";

import type {
  AccessRevealedSecret,
  AccessSectionId,
  TranslateFn,
} from "./accessKeysTypes";

export function CopyButton({
  t,
  value,
  label,
  variant = "secondary",
}: {
  t: TranslateFn;
  value: string | null | undefined;
  label?: string;
  variant?: "secondary" | "outline";
}) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");

  useEffect(() => {
    if (state === "idle") {
      return;
    }
    const timer = window.setTimeout(() => setState("idle"), 2400);
    return () => window.clearTimeout(timer);
  }, [state]);

  const copy = async () => {
    if (!value || !navigator.clipboard?.writeText) {
      setState("failed");
      return;
    }
    try {
      await navigator.clipboard.writeText(value);
      setState("copied");
    } catch {
      setState("failed");
    }
  };

  const resting = label ?? t("复制", "Copy");
  const caption =
    state === "copied"
      ? t("已复制", "Copied")
      : state === "failed"
        ? t("复制失败", "Copy failed")
        : resting;
  const stateClass =
    state === "copied"
      ? " nt-console-copy--ok"
      : state === "failed"
        ? " nt-console-copy--fail"
        : "";

  return (
    <button
      aria-label={resting}
      className={`nt-btn nt-btn--${variant} nt-console-copy${stateClass}`}
      onClick={() => {
        void copy();
      }}
      title={caption}
      type="button"
    >
      {state === "copied" ? (
        <Check aria-hidden="true" size={14} />
      ) : (
        <Copy aria-hidden="true" size={14} />
      )}
      <span>{caption}</span>
    </button>
  );
}

type StatTone = "default" | "emerald" | "danger" | "blue";

export function StatCard({
  label,
  value,
  hint,
  tone = "default",
}: {
  label: string;
  value: string;
  hint?: string;
  tone?: StatTone;
}) {
  const toneClass = tone === "default" ? "" : ` nt-pilot-stat-card--${tone}`;
  return (
    <article className={`nt-pilot-stat-card${toneClass}`}>
      <span>{label}</span>
      <strong>{value}</strong>
      {hint ? <small>{hint}</small> : null}
    </article>
  );
}

export function Section({
  id,
  title,
  icon,
  open,
  onToggle,
  children,
}: {
  id: AccessSectionId;
  title: string;
  icon: ReactNode;
  open: boolean;
  onToggle: (id: AccessSectionId) => void;
  children: ReactNode;
}) {
  const bodyId = `nt-access-section-${id}`;
  return (
    <section
      className={`nt-settings-section${open ? " nt-settings-section--open" : ""}`}
      data-section={id}
    >
      <h2 className="nt-settings-section__heading">
        <button
          aria-controls={bodyId}
          aria-expanded={open}
          className="nt-settings-section__trigger"
          onClick={() => onToggle(id)}
          type="button"
        >
          <span className="nt-settings-section__icon" aria-hidden="true">
            {icon}
          </span>
          <span>{title}</span>
          <span className="nt-settings-section__chevron" aria-hidden="true">
            <ChevronDown size={18} />
          </span>
        </button>
      </h2>
      {open ? (
        <div className="nt-settings-section__body" id={bodyId}>
          {children}
        </div>
      ) : null}
    </section>
  );
}

export function SecretBanner({
  t,
  secret,
  onDismiss,
}: {
  t: TranslateFn;
  secret: AccessRevealedSecret;
  onDismiss: () => void;
}) {
  return (
    <div className="nt-alert nt-alert--success" role="status">
      <div className="nt-console-secret">
        <strong>{secret.label}</strong>
        <code className="nt-console-secret__value">{secret.secret}</code>
        <p className="nt-pilot-stats-note">
          {secret.hint ??
            t(
              "该明文只会返回一次，请立即保存；关闭后无法再次查看。",
              "This plaintext is returned only once. Save it now; it cannot be shown again.",
            )}
        </p>
      </div>
      <div className="nt-console-secret__actions">
        <CopyButton label={t("复制明文", "Copy plaintext")} t={t} value={secret.secret} />
        <button className="nt-btn nt-btn--secondary" onClick={onDismiss} type="button">
          {t("我已保存", "I saved it")}
        </button>
      </div>
    </div>
  );
}
