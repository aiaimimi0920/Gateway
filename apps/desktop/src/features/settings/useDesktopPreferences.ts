import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

export type DesktopPreferences = {
  closeBehavior: "tray" | "exit"; retentionDays: number; logLimitMb: number;
  autostart: boolean; autostartSupported: boolean;
};
export function useDesktopPreferences() {
  const [value, setValue] = useState<DesktopPreferences | null>(null);
  const [pending, setPending] = useState(false);
  const [failed, setFailed] = useState(false);
  const mounted = useRef(false);
  const inFlight = useRef(false);
  useEffect(() => {
    let active = true;
    mounted.current = true;
    void invoke<DesktopPreferences>("load_gateway_desktop_preferences").then(result => { if (active) setValue(result); })
      .catch(() => { if (active) setFailed(true); });
    return () => { active = false; mounted.current = false; };
  }, []);
  const update = async (patch: Partial<DesktopPreferences>) => {
    if (!value || inFlight.current) return;
    const next = { ...value, ...patch };
    inFlight.current = true; setPending(true); setFailed(false);
    try {
      const saved = await invoke<DesktopPreferences>("save_gateway_desktop_preferences", {
        settings: { closeBehavior: next.closeBehavior, retentionDays: next.retentionDays, logLimitMb: next.logLimitMb }, autostart: next.autostart,
      });
      if (mounted.current) setValue(saved);
    } catch { if (mounted.current) setFailed(true); }
    finally { inFlight.current = false; if (mounted.current) setPending(false); }
  };
  return { value, pending, failed, update };
}
