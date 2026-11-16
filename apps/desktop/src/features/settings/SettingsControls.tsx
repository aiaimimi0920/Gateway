import type { ReactNode } from "react";

export function SettingRow({ id, label, children, note }: {
  id: string; label: string; children: ReactNode; note?: string;
}) {
  return <div className="nt-setting-row" data-setting-id={id} tabIndex={-1}>
    <div className="nt-setting-identity"><span className="nt-setting-label">{label}</span>
      {note && <small>{note}</small>}</div>
    <div className="nt-setting-control">{children}</div>
  </div>;
}
