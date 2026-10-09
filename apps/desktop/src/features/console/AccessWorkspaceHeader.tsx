import type { ReactNode } from "react";

import { SecretBanner } from "./AccessWorkspacePrimitives";
import type { AccessRevealedSecret, TranslateFn } from "./accessKeysTypes";

export function AccessWorkspaceHeader({
  t,
  notice,
  revealedSecret,
  onDismissSecret,
}: {
  t: TranslateFn;
  notice?: ReactNode;
  revealedSecret: AccessRevealedSecret | null;
  onDismissSecret: () => void;
}) {
  return (
    <>
      {notice}
      {revealedSecret ? (
        <SecretBanner onDismiss={onDismissSecret} secret={revealedSecret} t={t} />
      ) : null}
    </>
  );
}
