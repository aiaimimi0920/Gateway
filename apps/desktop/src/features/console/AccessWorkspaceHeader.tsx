import { RefreshCw } from "lucide-react";
import type { ReactNode } from "react";

import { SecretBanner } from "./AccessWorkspacePrimitives";
import type { AccessRevealedSecret, TranslateFn } from "./accessKeysTypes";

export function AccessWorkspaceHeader({
  t,
  notice,
  refreshing,
  onRefresh,
  revealedSecret,
  onDismissSecret,
}: {
  t: TranslateFn;
  notice?: ReactNode;
  refreshing: boolean;
  onRefresh: () => void;
  revealedSecret: AccessRevealedSecret | null;
  onDismissSecret: () => void;
}) {
  return (
    <>
      {notice}
      <div className="nt-console-toolbar">
        <p className="nt-copy">
          {t(
            "运营侧签发与吊销；终端用户自助流程仍在 Platform。明文令牌只返回一次。",
            "Operator-side issuing and revocation; end-user self-service stays in Platform. Plaintext tokens are returned only once.",
          )}
        </p>
        <button
          className="nt-btn nt-btn--outline"
          disabled={refreshing}
          onClick={onRefresh}
          type="button"
        >
          <RefreshCw size={15} aria-hidden="true" />
          <span>{refreshing ? t("刷新中…", "Refreshing…") : t("刷新", "Refresh")}</span>
        </button>
      </div>
      {revealedSecret ? (
        <SecretBanner onDismiss={onDismissSecret} secret={revealedSecret} t={t} />
      ) : null}
    </>
  );
}
