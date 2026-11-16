import { Field } from "@neuro/ui";
import type { ComponentProps } from "react";
import type { CatalogWorkflow } from "./providerCatalogWorkflow";

export type CatalogFieldsProps = {
  t(zh: string, en: string): string;
  workflow: CatalogWorkflow;
  update(patch: Partial<CatalogWorkflow>): void;
  locked: boolean;
};
export function CatalogField({ name, children, wide = false }: { name: string; children: ComponentProps<typeof Field>["children"]; wide?: boolean }) {
  return <div className={wide ? "nt-catalog-field--wide" : undefined}><Field label={name}>{children}</Field></div>;
}
