import { NvidiaBrandIcon } from "./NvidiaBrandIcon";
import { providerBrandAssets } from "./providerBrandAssets";
import "./ProviderBrandIcon.css";

type Props = { iconKey: string; fallback: string; title?: string };

/** Decorative beside the provider name; no remote URLs or user-supplied SVG. */
export function ProviderBrandIcon({ iconKey, fallback, title }: Props) {
  const asset = providerBrandAssets.get(iconKey);
  return <span
    className={`nt-provider-card__icon nt-provider-card__icon--${iconKey}${asset ? " nt-provider-card__icon--trademark" : ""}`}
    data-provider-icon={iconKey} title={title} aria-hidden="true"
  >
    {asset ? <img className="nt-provider-brand-icon" src={asset} alt="" width={28} height={28} draggable={false} />
      : iconKey === "nvidia" ? <NvidiaBrandIcon /> : fallback}
  </span>;
}
