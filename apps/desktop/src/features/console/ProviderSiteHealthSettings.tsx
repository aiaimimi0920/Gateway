import { Button, Dialog } from "@neuro/ui";
import { Activity } from "lucide-react";
import type { ComponentProps } from "react";
import { SiteHealthWorkspace } from "./SiteHealthWorkspace";
import "./ProviderSiteHealthSettings.css";

/** Mount monitoring only for the opened provider settings; closing aborts its requests. */
export function ProviderSiteHealthSettings(props: ComponentProps<typeof SiteHealthWorkspace> & { providerId: string; providerLabel: string }) {
  const { providerLabel, t, ...healthProps } = props;
  return <Dialog className="nt-provider-site-health" closeLabel={t("关闭", "Close")}
    title={t(`${providerLabel} · 站点健康`, `${providerLabel} · Site health`)}
    trigger={<Button>
      <Activity size={15} aria-hidden="true" />{t("站点健康", "Site health")}
    </Button>}>
    <div className="nt-provider-site-health__body"><SiteHealthWorkspace {...healthProps} t={t} initiallyOpen /></div>
  </Dialog>;
}
