import { AlertTriangle, Check, Info, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { createPortal } from "react-dom";

export type AppToastLevel = "error" | "warning" | "info" | "success";

type AppToastEntry = {
  id: number;
  level: AppToastLevel;
  message: string;
};

type AppToastListener = (entry: AppToastEntry) => void;

const listeners = new Set<AppToastListener>();
let nextToastId = 1;

const TOAST_LIFETIME_MS: Record<AppToastLevel, number> = {
  error: 5_200,
  warning: 4_200,
  info: 3_200,
  success: 3_200,
};

const TOAST_EXIT_MS = 220;

export function pushAppToast(level: AppToastLevel, message: string): void {
  const normalizedMessage = message.trim();
  if (!normalizedMessage) {
    return;
  }
  const entry = { id: nextToastId, level, message: normalizedMessage };
  nextToastId += 1;
  for (const listener of listeners) {
    listener(entry);
  }
}

function AppToastItem({
  entry,
  onRemove,
}: {
  entry: AppToastEntry;
  onRemove: (id: number) => void;
}) {
  const [exiting, setExiting] = useState(false);

  const dismiss = useCallback(() => {
    if (exiting) {
      return;
    }
    setExiting(true);
    window.setTimeout(() => onRemove(entry.id), TOAST_EXIT_MS);
  }, [entry.id, exiting, onRemove]);

  useEffect(() => {
    const timeoutId = window.setTimeout(dismiss, TOAST_LIFETIME_MS[entry.level]);
    return () => window.clearTimeout(timeoutId);
  }, [dismiss, entry.level]);

  const Icon =
    entry.level === "success"
      ? Check
      : entry.level === "warning"
        ? AlertTriangle
        : entry.level === "error"
          ? X
          : Info;
  const role = entry.level === "error" ? "alert" : "status";

  return (
    <button
      className={`nt-toast nt-toast--${entry.level}${exiting ? " nt-toast--exiting" : ""}`}
      type="button"
      role={role}
      aria-live={entry.level === "error" ? "assertive" : "polite"}
      aria-label={`Gateway console last action: ${entry.message}；点击关闭`}
      onClick={dismiss}
    >
      <span className="nt-toast__signal" aria-hidden="true" />
      <Icon className="nt-toast__icon" size={18} aria-hidden="true" />
      <span className="nt-toast__message">{entry.message}</span>
      <X className="nt-toast__dismiss" size={15} aria-hidden="true" />
    </button>
  );
}

export function AppToastViewport() {
  const [entries, setEntries] = useState<AppToastEntry[]>([]);

  useEffect(() => {
    const listener: AppToastListener = (entry) => {
      setEntries((current) => [...current, entry]);
    };
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const remove = useCallback((id: number) => {
    setEntries((current) => current.filter((entry) => entry.id !== id));
  }, []);

  if (typeof document === "undefined") {
    return null;
  }

  return createPortal(
    <div className="nt-toast-viewport" aria-label="Gateway notifications">
      {entries.map((entry) => (
        <AppToastItem key={entry.id} entry={entry} onRemove={remove} />
      ))}
    </div>,
    document.body,
  );
}
