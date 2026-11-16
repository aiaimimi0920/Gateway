import { Button, CopyButton as NeuroCopyButton, CopyValue, Disclosure, Notice } from "@neuro/ui";
import type { ReactNode } from "react";

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
  const resting = label ?? t("复制", "Copy");
  return (
    <NeuroCopyButton label={resting} variant={variant === "outline" ? "secondary" : variant} compact
      copiedLabel={t("已复制", "Copied")} errorLabel={t("复制失败", "Copy failed")} feedbackDuration={2400}
      onCopy={async () => {
        if (!value || !navigator.clipboard?.writeText) throw new Error("Clipboard API or copy value is unavailable");
        await navigator.clipboard.writeText(value);
      }} />
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
  return (
    <section className="nt-access-section" aria-label={title} data-section={id}>
      <Disclosure open={open} onOpenChange={() => onToggle(id)} title={
        <h2 className="nt-access-section__title"><span aria-hidden="true">{icon}</span>{title}</h2>
      }>
        {open ? <div className="nt-access-section__body">{children}</div> : null}
      </Disclosure>
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
    <Notice tone="success" className="nt-access-secret">
      <strong>{secret.label}</strong>
      <CopyValue value={secret.secret} label={t("复制明文", "Copy plaintext")}
        copiedLabel={t("已复制", "Copied")} errorLabel={t("复制失败", "Copy failed")} />
      <p>
        {secret.hint ??
          t(
            "该明文只会返回一次，请立即保存；关闭后无法再次查看。",
            "This plaintext is returned only once. Save it now; it cannot be shown again.",
          )}
      </p>
      <Button className="nt-access-secret__dismiss" onClick={onDismiss}>
        {t("我已保存", "I saved it")}
      </Button>
    </Notice>
  );
}
