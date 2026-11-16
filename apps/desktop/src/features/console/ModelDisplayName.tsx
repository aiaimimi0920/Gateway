import { createContext, useContext, useMemo, type ReactNode } from "react";
import type { ConsoleRouteDocument } from "../../api/contracts";
import { useDisplayPreferences, type DisplayPreferences } from "../settings/displayPreferences";
import { providerBrandLabel } from "./providerBrandLabel";
import { cardModelDisplayName } from "./modelDisplayCatalog";

type NameIndex = { aliases: Set<string>; providers: Map<string, { label: string; custom: Set<string> }> };
const empty: NameIndex = { aliases: new Set(), providers: new Map() };
const NameContext = createContext<{ index: NameIndex; mode: DisplayPreferences["providerNames"] }>({ index: empty, mode: "off" });
const record = (value: unknown): value is Record<string, unknown> => !!value && typeof value === "object" && !Array.isArray(value);

/** Publish only names, never credentials from the route document. */
export function buildModelNameIndex(document?: ConsoleRouteDocument | null): NameIndex {
  const index: NameIndex = { aliases: new Set(Object.keys(document?.aliases ?? {})), providers: new Map() };
  for (const provider of document?.providers ?? []) {
    if (!record(provider) || typeof provider.id !== "string") continue;
    const text = (key: string) => typeof provider[key] === "string" ? provider[key] as string : null;
    const custom = new Set<string>();
    for (const key of ["model_map", "model_map_targets"]) {
      if (record(provider[key])) for (const name of Object.keys(provider[key])) custom.add(name);
    }
    index.providers.set(provider.id, { label: providerBrandLabel({ providerId: provider.id,
      providerPreset: text("preset"), providerLabel: text("label"), vendorName: text("vendor_name") ?? text("vendorName") }), custom });
  }
  return index;
}

export function ModelNameProvider({ document, children }: { document?: ConsoleRouteDocument | null; children: ReactNode }) {
  const index = useMemo(() => buildModelNameIndex(document), [document]);
  const { value } = useDisplayPreferences();
  const context = useMemo(() => ({ index, mode: value.providerNames }), [index, value.providerNames]);
  return <NameContext.Provider value={context}>{children}</NameContext.Provider>;
}

export function formatModelName(model: string, providerIds: readonly string[], mode: DisplayPreferences["providerNames"], index: NameIndex): string {
  const providers = providerIds.map(id => index.providers.get(id));
  const custom = index.aliases.has(model) || providers.some(provider => provider?.custom.has(model));
  const name = custom ? model : cardModelDisplayName(model);
  if (mode === "off" || (mode === "except-custom" && custom)) return name;
  const labels = [...new Set(providerIds.map((id, i) => providers[i]?.label || id))];
  // Unknown ownership must not be guessed from the model's developer or API owned_by.
  return labels.length ? `${labels.join(" / ")} · ${name}` : model;
}

export function ModelDisplayName({ model, providerIds }: { model: string; providerIds: readonly string[] }) {
  const { index, mode } = useContext(NameContext);
  return <>{formatModelName(model, providerIds, mode, index)}</>;
}
