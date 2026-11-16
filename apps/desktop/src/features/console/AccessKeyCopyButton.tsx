import { CopyButton } from "@neuro/ui";
import type { TranslateFn } from "./accessKeysTypes";

export function AccessKeyCopyButton({
  disabled,
  onCopy,
  t,
}: {
  disabled: boolean;
  onCopy(): Promise<boolean>;
  t: TranslateFn;
}) {
  return (
    <CopyButton compact className="nt-key-copy" variant="quiet" label={t("复制 Key", "Copy Key")} disabled={disabled} feedbackDuration={2400}
      copiedLabel={t("已复制", "Copied")} errorLabel={t("复制失败", "Copy failed")}
      onCopy={async () => {
        if (!await onCopy()) throw new Error("Access key copy was rejected");
      }}>{t("复制", "Copy")}</CopyButton>
  );
}
