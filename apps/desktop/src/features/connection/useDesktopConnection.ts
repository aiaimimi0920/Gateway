import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";

export type ConnectionSettings = {
  mode: "local" | "existing";
  serverUrl: string;
};

export function useDesktopConnection(autoConnect = true, currentAddress = "") {
  const [settings, setSettingsValue] = useState<ConnectionSettings>({ mode: "local", serverUrl: "" });
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const [address, setAddress] = useState(currentAddress);
  const [operation, setOperation] = useState<"" | "test" | "apply">("");
  const [tested, setTested] = useState(false);
  const started = useRef(false);
  const inFlight = useRef(false);
  const setSettings = useCallback((next: ConnectionSettings) => {
    setSettingsValue(next); setTested(false); setError("");
  }, []);

  const connect = useCallback(async (next: ConnectionSettings) => {
    if (inFlight.current) return;
    inFlight.current = true;
    setOperation("apply"); setTested(false);
    setBusy(true);
    setError("");
    try {
      const url = await invoke<string>("connect_gateway", { settings: next });
      setAddress(url);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
      setOperation(""); inFlight.current = false;
    }
  }, []);

  const test = useCallback(async (next: ConnectionSettings) => {
    if (inFlight.current) return;
    inFlight.current = true; setBusy(true); setOperation("test"); setError(""); setTested(false);
    try { await invoke<string>("test_gateway_connection", { settings: next }); setTested(true); }
    catch (reason) { setError(String(reason)); }
    finally { inFlight.current = false; setBusy(false); setOperation(""); }
  }, []);

  useEffect(() => {
    let active = true;
    void invoke<ConnectionSettings>("load_gateway_connection").then((loaded) => {
      if (!active) return;
      setSettings(loaded);
      // StrictMode may mount effects twice. Only the surviving load launches a connection.
      if (autoConnect && !started.current) {
        started.current = true;
        void connect(loaded);
      }
      if (!autoConnect) setBusy(false);
    }).catch((reason: unknown) => {
      if (!active) return;
      setError(String(reason));
      setBusy(false);
    });
    return () => { active = false; };
  }, [autoConnect, connect, setSettings]);

  return { settings, setSettings, busy, error, address, connect, test, operation, tested };
}
