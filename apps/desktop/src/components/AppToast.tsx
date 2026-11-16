import { ToastViewport, type ToastEntry } from "@neuro/ui";
import { useCallback, useEffect, useState } from "react";

export type AppToastLevel = "error" | "warning" | "info" | "success";

type AppToastEntry = {
  id: number;
  level: AppToastLevel;
  message: string;
};

type AppToastListener = (entry: AppToastEntry) => void;

const listeners = new Set<AppToastListener>();
let nextToastId = 1;

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

  const notifications: ToastEntry[] = entries.map(entry => ({
    id: entry.id, message: entry.message, tone: entry.level === "error" ? "danger" : entry.level,
  }));
  return <ToastViewport entries={notifications} onRemove={remove} label="Gateway notifications" closeLabel="点击关闭" />;
}
