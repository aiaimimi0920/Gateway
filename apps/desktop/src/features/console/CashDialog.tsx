import * as Dialog from "@radix-ui/react-dialog";
import { useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import type { TranslateFn } from "./accessKeysTypes";
import "./cashBilling.css";

export function CashDialog({
  title,
  description,
  busy = false,
  wide = false,
  onClose,
  t,
  children,
}: {
  title: string;
  description: string;
  busy?: boolean;
  wide?: boolean;
  onClose(): void;
  t: TranslateFn;
  children: ReactNode;
}) {
  const trigger = useRef(
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null,
  );
  return (
    <Dialog.Root
      open
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
    >
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content
          className={`dialog-content nt-cash-dialog${wide ? " nt-cash-dialog--wide" : ""}`}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            if (trigger.current?.isConnected) trigger.current.focus();
          }}
          onEscapeKeyDown={(event) => {
            if (busy) event.preventDefault();
          }}
          onPointerDownOutside={(event) => {
            if (busy) event.preventDefault();
          }}
        >
          <div className="nt-cash-dialog__head">
            <Dialog.Title>{title}</Dialog.Title>
            <button
              type="button"
              className="nt-icon-action"
              aria-label={t("关闭", "Close")}
              disabled={busy}
              onClick={onClose}
            >
              <X size={17} />
            </button>
          </div>
          <Dialog.Description>{description}</Dialog.Description>
          {children}
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
