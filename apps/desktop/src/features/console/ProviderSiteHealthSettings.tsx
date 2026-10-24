import * as Dialog from "@radix-ui/react-dialog";
import { Activity, X } from "lucide-react";
import type { ComponentProps } from "react";
import { SiteHealthWorkspace } from "./SiteHealthWorkspace";
import "./ProviderSiteHealthSettings.css";

/** Mount monitoring only for the opened provider settings; closing aborts its requests. */
export function ProviderSiteHealthSettings(props: ComponentProps<typeof SiteHealthWorkspace> & { providerId: string; providerLabel: string }) {
  const { providerLabel, t, ...healthProps } = props;
  return <Dialog.Root>
    <Dialog.Trigger asChild><button className="nt-btn nt-btn--outline" type="button">
      <Activity size={15} aria-hidden="true" />{t("站点健康", "Site health")}
    </button></Dialog.Trigger>
    <Dialog.Portal><Dialog.Overlay className="dialog-overlay" />
      <Dialog.Content className="dialog-content nt-provider-site-health" aria-describedby={undefined}>
        <div className="nt-provider-site-health__header"><Dialog.Title>{t(`${providerLabel} · 站点健康`, `${providerLabel} · Site health`)}</Dialog.Title>
          <Dialog.Close asChild><button className="nt-icon-action" type="button" aria-label={t("关闭", "Close")}><X size={16} aria-hidden="true" /></button></Dialog.Close>
        </div>
        <div className="nt-provider-site-health__body"><SiteHealthWorkspace {...healthProps} t={t} initiallyOpen /></div>
      </Dialog.Content>
    </Dialog.Portal>
  </Dialog.Root>;
}
