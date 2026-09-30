import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";

export type ConnectionSettings = {
  mode: "local" | "existing";
  serverUrl: string;
};

export function useDesktopConnection() {
  const [settings, setSettings] = useState<ConnectionSettings>({ mode: "local", serverUrl: "" });
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [address, setAddress] = useState("");
  const started = useRef(false);

  const connect = useCallback(async (next: ConnectionSettings) => {
    setBusy(true);
    setError("");
    setAddress("");
    try {
      const url = await invoke<string>("connect_gateway", { settings: next });
      setAddress(url);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    let active = true;
    void invoke<ConnectionSettings>("load_gateway_connection").then((loaded) => {
      if (!active) return;
      setSettings(loaded);
      // StrictMode may mount effects twice. Only the surviving load launches a connection.
      if (!started.current) {
        started.current = true;
        void connect(loaded);
      }
    }).catch((reason: unknown) => {
      if (!active) return;
      setError(String(reason));
      setBusy(false);
    });
    return () => { active = false; };
  }, [connect]);

  return { settings, setSettings, busy, error, address, connect };
}
