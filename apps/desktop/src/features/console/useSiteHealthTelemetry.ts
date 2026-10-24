import { useEffect, useRef, useState } from "react";
import { z } from "zod";
import type { GatewayApiClient } from "../../api/client";
import {
  siteHealthResponseSchema,
  siteSnapshotSchema,
  type SiteHealthResponse,
} from "./siteHealth";

/** Poll and manual refresh share cancellation fences across host/token/provider/plan changes. */
export function useSiteHealthTelemetry(
  client: GatewayApiClient,
  managementToken: string | null,
  providerId: string,
  profileId: string | null,
  open: boolean,
) {
  const [reports, setReports] = useState<SiteHealthResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const manual = useRef<AbortController | null>(null);
  useEffect(() => {
    setBusy(false);
    setError(null);
    return () => {
      manual.current?.abort();
      manual.current = null;
    };
  }, [client, managementToken, providerId, profileId, open]);
  useEffect(() => {
    setReports(null);
    if (!open || !managementToken || !providerId) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout>;
    const load = async () => {
      try {
        const result = await client.request(
          "/v1/internal/gateway/site-health",
          siteHealthResponseSchema,
          { managementToken, signal: controller.signal },
        );
        if (!controller.signal.aborted) {
          setReports(result);
          setError(null);
        }
      } catch (error) {
        if (!controller.signal.aborted) setError(String(error));
      }
      if (!controller.signal.aborted)
        timer = setTimeout(() => void load(), 15000);
    };
    void load();
    return () => {
      controller.abort();
      clearTimeout(timer);
    };
  }, [open, managementToken, client, refresh, providerId]);
  const detect = () => {
    if (busy || !managementToken || !providerId || !open) return;
    const controller = new AbortController();
    manual.current?.abort();
    manual.current = controller;
    const profile =
      profileId === null ? "" : `/profiles/${encodeURIComponent(profileId)}`;
    setBusy(true);
    void client
      .request(
        `/v1/internal/gateway/site-health/${encodeURIComponent(providerId)}${profile}/refresh`,
        z.object({ snapshot: siteSnapshotSchema }),
        { method: "POST", managementToken, signal: controller.signal },
      )
      .then(() => {
        if (!controller.signal.aborted) setRefresh((n) => n + 1);
      })
      .catch((error) => {
        if (!controller.signal.aborted) setError(String(error));
      })
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });
  };
  const item = reports?.items.find((r) => r.provider_id === providerId);
  const report =
    profileId === null
      ? item?.snapshot
      : item?.profiles.find((p) => p.profile_id === profileId)?.snapshot;
  return { report, error, busy, detect };
}
