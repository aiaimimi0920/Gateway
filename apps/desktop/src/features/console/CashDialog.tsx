import { Dialog } from "@neuro/ui";
import type { ReactNode } from "react";
import type { TranslateFn } from "./accessKeysTypes";
import "./cashBilling.css";

export function CashDialog({
  title,
  description,
  busy = false,
  wide = false,
  className = "",
  onClose,
  t,
  children,
}: {
  title: string;
  description: string;
  busy?: boolean;
  wide?: boolean;
  className?: string;
  onClose(): void;
  t: TranslateFn;
  children: ReactNode;
}) {
  return (
    <Dialog
      open
      title={title}
      description={description}
      busy={busy}
      closeLabel={t("关闭", "Close")}
      className={`nt-cash-dialog${wide ? " nt-cash-dialog--wide" : ""} ${className}`}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      {children}
    </Dialog>
  );
}
