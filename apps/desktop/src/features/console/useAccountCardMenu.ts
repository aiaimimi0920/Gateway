import { Copy, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import type { AccountCardMenuItem } from "./accountCardTypes";

export const ACCOUNT_CARD_MENU_ITEMS: readonly AccountCardMenuItem[] = [
  { id: "probe", icon: Play, label: ["账号测试", "Test account"] },
  { id: "duplicate", icon: Copy, label: ["复制账号", "Duplicate account"] },
];

/** Supplies shared outside-pointer, Escape, and focus-restoration behavior. */
export function useAccountCardMenu() {
  const [activeMenuKey, setActiveMenuKey] = useState<string | null>(null);
  const menuTriggerRefs = useRef(new Map<string, HTMLButtonElement>());

  useEffect(() => {
    if (!activeMenuKey) {
      return;
    }

    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!(event.target instanceof Element)) {
        return;
      }
      const menuRoot = event.target.closest<HTMLElement>(".nt-pilot-menu");
      if (menuRoot?.dataset.pilotMenuKey === activeMenuKey) {
        return;
      }
      setActiveMenuKey(null);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") {
        return;
      }
      event.preventDefault();
      const trigger = menuTriggerRefs.current.get(activeMenuKey);
      setActiveMenuKey(null);
      queueMicrotask(() => trigger?.focus());
    };

    document.addEventListener("pointerdown", closeOnOutsidePointer, true);
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer, true);
      document.removeEventListener("keydown", closeOnEscape);
    };
  }, [activeMenuKey]);

  const registerMenuTrigger = (key: string, node: HTMLButtonElement | null) => {
    if (node) {
      menuTriggerRefs.current.set(key, node);
    } else {
      menuTriggerRefs.current.delete(key);
    }
  };

  return { activeMenuKey, setActiveMenuKey, menuTriggerRefs, registerMenuTrigger };
}
