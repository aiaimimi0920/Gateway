import { useEffect, useMemo, useRef, useState } from "react";
import { lumaFilesSchema, type LumaFile, type LumaFileOperation } from "../../api/console/luma-files";
import { presigned, uploadForm, validUpload, type LumaUpload } from "./lumaFileUpload";
import type { LumaOAuthProps } from "./useLumaOAuth";

export type LumaFilesProps = Pick<LumaOAuthProps, "client" | "managementToken" | "secretGrant" | "providerId" | "credentialId"
  | "expectedRevision" | "disabled" | "onRequestSecretAccess">;

// FileReader and HTTP share cancellation, including changes of account, grant or revision.
function readFile(file: File, signal: AbortSignal): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    const abort = () => { reader.abort(); reject(new Error("aborted")); };
    if (signal.aborted) { reject(new Error("aborted")); return; }
    signal.addEventListener("abort", abort, { once: true });
    reader.onloadend = () => {
      signal.removeEventListener("abort", abort);
      if (reader.error || typeof reader.result !== "string") reject(new Error("file read"));
      else resolve(reader.result.slice(reader.result.indexOf(",") + 1));
    };
    reader.readAsDataURL(file);
  });
}

export function useLumaFiles(props: LumaFilesProps) {
  const scope = useMemo(() => ({ client: props.client, token: props.managementToken, grant: props.secretGrant,
    providerId: props.providerId, credentialId: props.credentialId, revision: props.expectedRevision, disabled: props.disabled }),
  [props.client, props.managementToken, props.secretGrant, props.providerId, props.credentialId, props.expectedRevision, props.disabled]);
  const current = useRef(scope); current.current = scope;
  const pending = useRef<AbortController | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<"request" | "file" | null>(null);
  const [result, setResult] = useState<{ scope: typeof scope; rows: LumaFile[]; cursor: string | null } | null>(null);
  const [selected, setSelected] = useState("");
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  useEffect(() => {
    setResult(null); setSelected(""); setConfirmDelete(null); setError(null); setBusy(false);
    return () => { pending.current?.abort(); pending.current = null; };
  }, [scope]);
  const enabled = !scope.disabled && !!scope.token && !!scope.credentialId && !!scope.revision;
  const page = result?.scope === scope ? result : null;
  const file = page?.rows.find((row) => row.id === selected);
  const perform = async (operation: LumaFileOperation | LumaUpload) => {
    if (!enabled || pending.current) return;
    if (!scope.grant) {
      try { await props.onRequestSecretAccess(); } catch { if (current.current === scope) setError("request"); }
      return;
    }
    if (operation.action === "delete" && (confirmDelete !== operation.id || file?.id !== operation.id)) return;
    if (operation.action === "upload" && "file" in operation) {
      if (!validUpload(operation)) {
        setError("file"); return;
      }
    }
    const controller = new AbortController(); pending.current = controller;
    setBusy(true); setError(null); setConfirmDelete(null);
    try {
      const binding = { providerId: scope.providerId, credentialId: scope.credentialId!, expectedRevision: scope.revision! };
      const form = operation.action === "upload" && "file" in operation && presigned(operation) ? uploadForm(operation, binding) : null;
      const prepared = !form && operation.action === "upload" && "file" in operation
        ? { action: "upload" as const, filename: operation.file.name, purpose: operation.purpose, data: await readFile(operation.file, controller.signal) } : operation;
      if (controller.signal.aborted || current.current !== scope) return;
      const response = await scope.client.request("/v1/internal/gateway/console/luma-agents-files", lumaFilesSchema, {
        method: "POST", cache: "no-store", managementToken: scope.token, secretGrant: scope.grant, signal: controller.signal,
        // Upstream file authorization can return 401/403; it does not invalidate the console session.
        notifyAuthenticationFailure: false,
        body: form ?? { ...binding, operation: prepared },
      });
      if (controller.signal.aborted || current.current !== scope) return;
      if (response.revision !== scope.revision || response.providerId !== scope.providerId || response.credentialId !== scope.credentialId
        || response.result.action !== operation.action) throw new Error("binding");
      const data = response.result;
      if (data.action === "list") {
        setResult({ scope, rows: data.data, cursor: data.has_more ? data.next_cursor : null }); setSelected(data.data[0]?.id ?? "");
      } else if (data.action === "delete") {
        if (!("id" in operation) || data.id !== operation.id) throw new Error("file binding");
        const rows = page?.rows.filter((row) => row.id !== data.id) ?? [];
        setResult({ scope, rows, cursor: page?.cursor ?? null }); setSelected(rows[0]?.id ?? "");
      } else if (data.action === "get") {
        if (!("id" in operation) || data.file.id !== operation.id) throw new Error("file binding");
        setResult({ scope, rows: (page?.rows ?? []).map((row) => row.id === data.file.id ? data.file : row), cursor: page?.cursor ?? null });
      } else {
        setResult({ scope, rows: [data.file], cursor: null }); setSelected(data.file.id);
      }
    } catch {
      if (!controller.signal.aborted && current.current === scope) {
        setError("request");
        if (operation.action === "get") setResult(null);
      }
    } finally {
      if (pending.current === controller) { pending.current = null; setBusy(false); }
    }
  };
  const stop = () => { pending.current?.abort(); pending.current = null; setBusy(false); setError("request"); };
  return { scope, enabled, busy, error, page, file, selected, confirmDelete, setConfirmDelete, perform, stop,
    select: (id: string) => { setSelected(id); setConfirmDelete(null); } };
}
