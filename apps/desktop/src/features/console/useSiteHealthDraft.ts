import { useCallback, useEffect, useState } from "react";
import type {
  ConsoleRouteConfigResponse,
  ConsoleRouteDocument,
} from "../../api/contracts";
import { applySiteHealth, type SiteHealthConfig } from "./siteHealth";
import { parseRouteDocument } from "./routeDocument";
import {
  healthSecretTarget,
  validateHealthSecretChange,
  type SiteHealthSecretChange,
  type SiteHealthSecretEdit,
} from "./siteHealthSecretDraft";

type Options = {
  editorText: string;
  routeConfig: ConsoleRouteConfigResponse | null;
  hasSecretAccess: boolean;
  replace(document: ConsoleRouteDocument, sync?: boolean): void;
  setError(error: string | null): void;
  requestSecretAccess(): void;
};

/** Keep monitor tokens in short-lived secret drafts, never editorText or plain policy JSON. */
export function useSiteHealthDraft({
  editorText,
  routeConfig,
  hasSecretAccess,
  replace,
  setError,
  requestSecretAccess,
}: Options) {
  const [siteHealthSecretEdits, setEdits] = useState<SiteHealthSecretEdit[]>(
    [],
  );
  useEffect(() => {
    setEdits([]);
  }, [routeConfig]);
  useEffect(() => {
    if (!hasSecretAccess) setEdits([]);
  }, [hasSecretAccess]);
  const applySiteHealthDraft = useCallback(
    (
      id: string,
      config: SiteHealthConfig | null,
      changes: SiteHealthSecretChange[] = [],
    ) => {
      try {
        if (
          changes.some((change) => change.operation !== "keep") &&
          !hasSecretAccess
        ) {
          requestSecretAccess();
          throw new Error("编辑监控密钥前需要先确认敏感信息访问权限。");
        }
        const next = applySiteHealth(
          parseRouteDocument(editorText),
          id,
          config,
        );
        const edits: SiteHealthSecretEdit[] = changes.flatMap((change) => {
          if (change.operation === "keep") return [];
          validateHealthSecretChange(change);
          const target = healthSecretTarget(next, id, change.profileId);
          if (!target) throw new Error("监控方案不存在，不能保存密钥。");
          return [{ ...change, providerId: id, audience: target.audience }];
        });
        setEdits((current) => [
          ...current.filter(
            (edit) =>
              healthSecretTarget(next, edit.providerId, edit.profileId) &&
              !(
                edit.providerId === id &&
                changes.some((change) => change.profileId === edit.profileId)
              ),
          ),
          ...edits,
        ]);
        replace(next, true);
        setError(null);
      } catch (error) {
        setError(error instanceof Error ? error.message : String(error));
      }
    },
    [editorText, hasSecretAccess, replace, requestSecretAccess, setError],
  );
  return { siteHealthSecretEdits, applySiteHealth: applySiteHealthDraft };
}
