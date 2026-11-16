import { Button, Dialog } from "@neuro/ui";
import type { ReactNode } from "react";

/** Query diagnostics overlay the list rather than changing its position. */
export function OperationsDetail({ title, children, className }: { title: string; children: ReactNode; className?: string }) {
  return <div className={className}><Dialog className="nt-operations-drawer" title={title} trigger={<Button compact>{title}</Button>}>{children}</Dialog></div>;
}
