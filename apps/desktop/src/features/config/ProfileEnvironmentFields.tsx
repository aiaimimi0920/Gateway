import { Button, EmptyState, Field, Input } from "@neuro/ui";
import type { GatewayEnvEntry } from "../../lib/types";

export function ProfileEnvironmentFields({ entries, errors, onChange }: {
  entries: GatewayEnvEntry[];
  errors: Record<number, string>;
  onChange(entries: GatewayEnvEntry[]): void;
}) {
  const update = (index: number, patch: Partial<GatewayEnvEntry>) => onChange(entries.map((entry, entryIndex) => entryIndex === index ? { ...entry, ...patch } : entry));
  return <>
    <div className="nt-section__head"><h2>附加环境变量</h2>
      <Button onClick={() => onChange([...entries, { key: "", value: "" }])}>添加变量</Button>
    </div>
    {!entries.length ? <EmptyState title="暂无附加环境变量" /> : <div className="gateway-profile-env">
      {entries.map((entry, index) =>
        // Entries have no persisted ID and cannot be reordered; editing a key must not replace its input.
        <div className="gateway-profile-env__row" key={index}>
          <Field label={`变量名 ${index + 1}`} error={errors[index]}>
            <Input placeholder="KEY" value={entry.key} onChange={event => update(index, { key: event.currentTarget.value })} />
          </Field>
          <Field label={`值 ${index + 1}`}><Input placeholder="VALUE" value={entry.value} onChange={event => update(index, { value: event.currentTarget.value })} /></Field>
          <Button variant="quiet" aria-label={`移除变量 ${index + 1}`} onClick={() => onChange(entries.filter((_, entryIndex) => entryIndex !== index))}>移除</Button>
        </div>)}
    </div>}
  </>;
}
