import { useLayoutEffect, useRef, useState } from "react";

// The native top layer escapes the dialog's scroll clipping without changing form geometry.
export function useKeyGroupDropdown(disabled: boolean) {
  const [open, setOpen] = useState(false);
  const trigger = useRef<HTMLButtonElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (disabled) setOpen(false);
    if (!open || disabled) return;
    const button = trigger.current;
    const popup = panel.current;
    if (!button || !popup) return;
    const position = () => {
      const rect = button.getBoundingClientRect();
      const width = Math.min(Math.max(rect.width, 280), window.innerWidth - 24);
      const below = window.innerHeight - rect.bottom - 12;
      const above = rect.top - 12;
      const downward = below >= Math.min(280, above);
      const height = Math.min(280, Math.max(0, downward ? below : above));
      Object.assign(popup.style, {
        width: `${width}px`,
        maxHeight: `${height}px`,
        left: `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`,
        top: downward ? `${rect.bottom + 6}px` : "auto",
        bottom: downward ? "auto" : `${window.innerHeight - rect.top + 6}px`,
      });
    };
    position();
    popup.showPopover?.();
    popup.querySelector<HTMLElement>("input, button:not(:disabled)")?.focus();
    const outside = (event: Event) => {
      if (event.target instanceof Node && !popup.contains(event.target) && !button.contains(event.target)) setOpen(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      // Consume the first Escape before Radix closes the containing editor.
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      button.focus();
    };
    const blur = (event: FocusEvent) => {
      if (!(event.relatedTarget instanceof Node) || (!popup.contains(event.relatedTarget) && !button.contains(event.relatedTarget))) setOpen(false);
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    window.addEventListener("keydown", escape, true);
    popup.addEventListener("focusout", blur);
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    return () => {
      popup.hidePopover?.();
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      window.removeEventListener("keydown", escape, true);
      popup.removeEventListener("focusout", blur);
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
    };
  }, [open, disabled]);
  return { open, setOpen, trigger, panel };
}
