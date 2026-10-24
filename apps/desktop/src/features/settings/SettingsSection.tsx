import { useId, useState, type ReactNode } from "react";
import { ChevronDown } from "lucide-react";

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
  const [open, setOpen] = useState(defaultOpen);
  const bodyId = useId();

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

  return (
    <section className={`nt-settings-section${open ? " nt-settings-section--open" : ""}`}>
      <h2 className="nt-settings-section__heading">
        <button
          className="nt-settings-section__trigger"
          type="button"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={() => setOpen((current) => !current)}
        >
          <span className="nt-settings-section__icon" aria-hidden="true">
            {icon}
          </span>
          <span>{label}</span>
          <span className="nt-settings-section__chevron" aria-hidden="true">
            <ChevronDown size={18} />
          </span>
        </button>
      </h2>
      {open ? (
        <div className="nt-settings-section__body" id={bodyId}>
          {children}
        </div>
      ) : null}
    </section>
  );
}
