import { Copy } from "lucide-react";
import { pushAppToast } from "../../components/AppToast";
import type { TranslateFn } from "./accountsLedgerTypes";

type Props = { providerLabel: string; label: string; value?: string | null; t: TranslateFn };

/** Only the copy action accesses the real endpoint; it is never expanded or navigated. */
export function ProviderStorageEndpoints({ providerLabel, label, value, t }: Props) {
  const copy = async () => {
    if (!value) return;
    if (!navigator.clipboard?.writeText) {
      pushAppToast("warning", t("当前环境不支持剪贴板写入。", "Clipboard access is unavailable."));
      return;
    }
    try {
      await navigator.clipboard.writeText(value);
      pushAppToast("success", t(`已复制${label}。`, `${label} copied.`));
    } catch {
      pushAppToast("error", t("复制失败，请检查剪贴板权限。", "Copy failed. Check clipboard permissions."));
    }
  };
  return <div className="nt-provider-lifecycle__endpoint">
    <span>{label}</span><span aria-hidden="true">{value ? "•••" : "—"}</span>
    <button className="nt-icon-action" type="button" disabled={!value}
      aria-label={t(`复制 ${providerLabel} ${label} API`, `Copy ${providerLabel} ${label} API`)}
      title={t(`复制${label} API`, `Copy ${label} API`)} onClick={() => void copy()}>
      <Copy size={14} aria-hidden="true" />
    </button>
  </div>;
}
