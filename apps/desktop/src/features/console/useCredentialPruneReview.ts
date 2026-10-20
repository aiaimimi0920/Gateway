// No destructive action on open. Identity epochs suppress stale server/session completions.
import { useCallback, useLayoutEffect, useRef, useState } from "react";
import type { ConsoleApi } from "../../api/console";
import type { CredentialPrunePreview, CredentialPruneReceipt } from "../../api/schemas/credential-prune";

export type CredentialPruneReview = {
  providerId: string;
  preview: CredentialPrunePreview | null;
  receipt: CredentialPruneReceipt | null;
  error: string | null;
  loading: boolean;
  applying: boolean;
};

type Options = {
  api: ConsoleApi;
  managementToken: string | null;
  draftDirty: boolean;
  refresh(): Promise<void>;
  t(zh: string, en: string): string;
};

export function useCredentialPruneReview({ api, managementToken, draftDirty, refresh, t }: Options) {
  const [review, setReview] = useState<CredentialPruneReview | null>(null);
  const scope = useRef({ active: false, epoch: 0, sequence: 0, api, managementToken, busy: false,
    preview: null as CredentialPrunePreview | null });
  const dirty = useRef(draftDirty);
  const renderedEpoch = scope.current.epoch;
  const renderedSequence = scope.current.sequence;
  dirty.current = draftDirty;
  useLayoutEffect(() => {
    Object.assign(scope.current, { active: true, api, managementToken, busy: false, preview: null });
    setReview(null);
    return () => { scope.current.active = false; scope.current.epoch += 1; };
  }, [api, managementToken]);
  const active = useCallback(() => scope.current.active && scope.current.api === api &&
    scope.current.managementToken === managementToken, [api, managementToken]);
  const close = useCallback(() => {
    if (!active() || scope.current.busy) return;
    scope.current.sequence += 1;
    scope.current.preview = null;
    setReview(null);
  }, [active]);
  const open = useCallback(async (providerId: string) => {
    if (!active() || !managementToken || dirty.current || scope.current.busy) return;
    const { epoch } = scope.current;
    const sequence = ++scope.current.sequence;
    scope.current.preview = null;
    const current = () => active() && scope.current.epoch === epoch && scope.current.sequence === sequence;
    setReview({ providerId, preview: null, receipt: null, error: null, loading: true, applying: false });
    try {
      const result = await api.previewCredentialPrune(managementToken, providerId);
      if (current()) {
        scope.current.preview = result.preview;
        setReview({ providerId, preview: result.preview, receipt: null, error: null, loading: false, applying: false });
      }
    } catch (cause) {
      if (current()) setReview({ providerId, preview: null, receipt: null, error: cause instanceof Error ? cause.message : String(cause), loading: false, applying: false });
    }
  }, [active, api, managementToken]);
  const apply = useCallback(async (credentialIds: string[]) => {
    if (!active() || !managementToken || dirty.current || scope.current.busy || !review?.preview ||
      scope.current.epoch !== renderedEpoch || scope.current.sequence !== renderedSequence ||
      scope.current.preview !== review.preview || review.receipt || review.error) return;
    const preview = review.preview;
    if (!credentialIds.length || credentialIds.length > 128 || new Set(credentialIds).size !== credentialIds.length ||
      credentialIds.some(id => !preview.items.some(item => item.credentialId === id))) return;
    const { epoch, sequence } = scope.current;
    const current = () => active() && scope.current.epoch === epoch && scope.current.sequence === sequence;
    scope.current.busy = true;
    scope.current.preview = null;
    setReview(current => current && { ...current, applying: true, error: null });
    try {
      const result = await api.applyCredentialPrune(managementToken, review.providerId, {
        expectedRevision: preview.revision, previewDigest: preview.digest, credentialIds,
      });
      if (!current()) return;
      setReview(value => value && { ...value, receipt: result.receipt, applying: false });
      try { await refresh(); } catch {
        if (current()) setReview(value => value && { ...value, error: t("刷新失败，请重新加载后核对结果。", "Refresh failed; reload to verify the result.") });
      }
    } catch (cause) {
      if (!current()) return;
      // A lost HTTP response can follow a durable commit. Never enable blind retry.
      setReview(value => value && { ...value, applying: false, error: `${cause instanceof Error ? cause.message : String(cause)} ${t("请刷新并重新预览，不要直接重试。", "Refresh and inspect again; do not retry blindly.")}` });
      try { await refresh(); } catch { /* The original uncertain outcome remains visible. */ }
    } finally {
      if (current()) scope.current.busy = false;
    }
  }, [active, api, managementToken, refresh, renderedEpoch, renderedSequence, review, t]);
  return { credentialPruneReview: review, closeCredentialPruneReview: close, applyCredentialPruneReview: apply,
    handlePruneCredentialPool: open, credentialPoolPruneBusy: review?.loading || review?.applying ? review.providerId : null };
}
