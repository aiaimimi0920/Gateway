import type { ReactNode } from "react";
import type { CatalogWorkflow } from "./providerCatalogWorkflow";

export type CatalogFieldsProps = {
  t(zh: string, en: string): string;
  workflow: CatalogWorkflow;
  update(patch: Partial<CatalogWorkflow>): void;
  locked: boolean;
};
export function CatalogField({ name, children, wide = false }: { name: string; children: ReactNode; wide?: boolean }) {
  return <label className={`nt-catalog-field${wide ? " nt-catalog-field--wide" : ""}`}><span>{name}</span>{children}</label>;
}
