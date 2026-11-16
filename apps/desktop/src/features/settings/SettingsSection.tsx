import type { ReactNode } from "react";
import { Disclosure } from "@neuro/ui";

type SettingsSectionProps = {
  label: string;
  icon: ReactNode;
  defaultOpen?: boolean;
  collapsible?: boolean;
  children: ReactNode;
};

/**
 * One collapsible settings row, shaped like Loom's settings accordion: a
 * hairline-separated trigger (icon + label + chevron) with an indented body.
 * Both shells reuse it so the launcher and the web console read the same.
 */
export function SettingsSection({ label, icon, defaultOpen = true, collapsible = true, children }: SettingsSectionProps) {
  if (!collapsible) {
    return (
      <section className="nt-settings-section nt-settings-section--static">
        <h2 className="nt-settings-section__heading">
          <span className="nt-settings-section__icon" aria-hidden="true">{icon}</span>
          {label}
        </h2>
        <div className="nt-settings-section__body">{children}</div>
      </section>
    );
  }

  return <Disclosure title={<span><span aria-hidden="true">{icon}</span> {label}</span>} defaultOpen={defaultOpen}>{children}</Disclosure>;
}
