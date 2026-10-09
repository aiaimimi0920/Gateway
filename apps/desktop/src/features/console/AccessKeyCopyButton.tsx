import { Check, Copy } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { TranslateFn } from "./accessKeysTypes";

export function AccessKeyCopyButton({
  disabled,
  onCopy,
  t,
}: {
  disabled: boolean;
  onCopy(): Promise<boolean>;
  t: TranslateFn;
}) {
  const [state, setState] = useState<"idle" | "copying" | "copied" | "failed">(
    "idle",
  );
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    if (state !== "copied" && state !== "failed") return;
    const timer = window.setTimeout(() => setState("idle"), 2400);
    return () => window.clearTimeout(timer);
  }, [state]);
  return (
    <button
      className="nt-btn nt-btn--outline nt-console-copy"
      type="button"
      aria-label={t("复制 Key", "Copy Key")}
      disabled={disabled || state === "copying"}
      onClick={async () => {
        setState("copying");
        try {
          const ok = await onCopy();
          if (mounted.current) setState(ok ? "copied" : "failed");
        } catch {
          if (mounted.current) setState("failed");
        }
      }}
    >
      {state === "copied" ? (
        <Check size={14} aria-hidden="true" />
      ) : (
        <Copy size={14} aria-hidden="true" />
      )}
      {state === "copying"
        ? t("复制中…", "Copying…")
        : state === "copied"
          ? t("已复制", "Copied")
          : state === "failed"
            ? t("复制失败", "Copy failed")
            : t("复制 Key", "Copy Key")}
    </button>
  );
}
